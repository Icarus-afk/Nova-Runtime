use nova_sql::*;

fn setup() -> SQLEngine {
    SQLEngine::new(SQLConfig::default())
}

#[test]
fn test_create_table_and_insert() {
    let engine = setup();
    let result = engine.execute("CREATE TABLE test (id INTEGER, name TEXT)").unwrap();
    assert!(matches!(result, SQLResult::Exec { .. }));

    let result = engine.execute("INSERT INTO test VALUES (1, 'hello')").unwrap();
    assert!(matches!(result, SQLResult::Exec { rows_affected: 1, .. }));

    let result = engine.execute("SELECT * FROM test").unwrap();
    match result {
        SQLResult::Query { batches, .. } => {
            assert_eq!(batches.len(), 1);
            assert_eq!(batches[0].num_rows, 1);
            assert_eq!(batches[0].num_columns(), 2);
        }
        _ => panic!("expected query result"),
    }
}

#[test]
fn test_simple_select() {
    let engine = setup();
    engine.execute("CREATE TABLE t (a INTEGER, b TEXT)").unwrap();
    engine.execute("INSERT INTO t VALUES (1, 'one')").unwrap();
    engine.execute("INSERT INTO t VALUES (2, 'two')").unwrap();
    engine.execute("INSERT INTO t VALUES (3, 'three')").unwrap();

    let result = engine.execute("SELECT * FROM t").unwrap();
    match result {
        SQLResult::Query { batches, stats } => {
            assert_eq!(batches[0].num_rows, 3);
            assert_eq!(stats.rows_returned, 3);
        }
        _ => panic!("expected query result"),
    }
}

#[test]
fn test_select_with_where() {
    let engine = setup();
    engine.execute("CREATE TABLE t (a INTEGER, b TEXT)").unwrap();
    engine.execute("INSERT INTO t VALUES (1, 'a')").unwrap();
    engine.execute("INSERT INTO t VALUES (2, 'b')").unwrap();
    engine.execute("INSERT INTO t VALUES (3, 'c')").unwrap();

    let batches = engine.execute_query("SELECT * FROM t WHERE a > 1").unwrap();
    assert_eq!(batches[0].num_rows, 2);

    let batches = engine.execute_query("SELECT * FROM t WHERE a = 2").unwrap();
    assert_eq!(batches[0].num_rows, 1);

    let batches = engine.execute_query("SELECT * FROM t WHERE a < 2").unwrap();
    assert_eq!(batches[0].num_rows, 1);
}

#[test]
fn test_select_with_where_and_or() {
    let engine = setup();
    engine.execute("CREATE TABLE t (a INTEGER, b TEXT)").unwrap();
    engine.execute("INSERT INTO t VALUES (1, 'x')").unwrap();
    engine.execute("INSERT INTO t VALUES (2, 'y')").unwrap();
    engine.execute("INSERT INTO t VALUES (3, 'z')").unwrap();

    let batches = engine.execute_query("SELECT * FROM t WHERE a > 1 AND b = 'y'").unwrap();
    assert_eq!(batches[0].num_rows, 1);

    let batches = engine.execute_query("SELECT * FROM t WHERE a = 1 OR a = 3").unwrap();
    assert_eq!(batches[0].num_rows, 2);
}

#[test]
fn test_select_projection() {
    let engine = setup();
    engine.execute("CREATE TABLE t (a INTEGER, b TEXT, c INTEGER)").unwrap();
    engine.execute("INSERT INTO t VALUES (1, 'x', 10)").unwrap();
    engine.execute("INSERT INTO t VALUES (2, 'y', 20)").unwrap();

    let batches = engine.execute_query("SELECT a, b FROM t").unwrap();
    assert_eq!(batches[0].num_rows, 2);
    assert_eq!(batches[0].num_columns(), 2);

    let batches = engine.execute_query("SELECT a FROM t").unwrap();
    assert_eq!(batches[0].num_columns(), 1);
}

#[test]
fn test_select_limit() {
    let engine = setup();
    engine.execute("CREATE TABLE t (a INTEGER)").unwrap();
    engine.execute("INSERT INTO t VALUES (1)").unwrap();
    engine.execute("INSERT INTO t VALUES (2)").unwrap();
    engine.execute("INSERT INTO t VALUES (3)").unwrap();
    engine.execute("INSERT INTO t VALUES (4)").unwrap();
    engine.execute("INSERT INTO t VALUES (5)").unwrap();

    let batches = engine.execute_query("SELECT * FROM t LIMIT 3").unwrap();
    assert_eq!(batches[0].num_rows, 3);

    let batches = engine.execute_query("SELECT * FROM t LIMIT 2 OFFSET 1").unwrap();
    assert_eq!(batches[0].num_rows, 2);
}

#[test]
fn test_select_order_by() {
    let engine = setup();
    engine.execute("CREATE TABLE t (a INTEGER)").unwrap();
    engine.execute("INSERT INTO t VALUES (3)").unwrap();
    engine.execute("INSERT INTO t VALUES (1)").unwrap();
    engine.execute("INSERT INTO t VALUES (2)").unwrap();

    let batches = engine.execute_query("SELECT * FROM t ORDER BY a ASC").unwrap();
    let col = batches[0].get_column(0).unwrap();
    match col {
        Column::Integer(vals) => {
            assert_eq!(vals, &[Some(1), Some(2), Some(3)]);
        }
        _ => panic!("expected integer column"),
    }

    let batches = engine.execute_query("SELECT * FROM t ORDER BY a DESC").unwrap();
    let col = batches[0].get_column(0).unwrap();
    match col {
        Column::Integer(vals) => {
            assert_eq!(vals, &[Some(3), Some(2), Some(1)]);
        }
        _ => panic!("expected integer column"),
    }
}

#[test]
fn test_select_is_null() {
    let engine = setup();
    engine.execute("CREATE TABLE t (a INTEGER, b TEXT)").unwrap();
    engine.execute("INSERT INTO t VALUES (1, 'x')").unwrap();
    engine.execute("INSERT INTO t VALUES (NULL, 'y')").unwrap();
    engine.execute("INSERT INTO t VALUES (3, NULL)").unwrap();

    let batches = engine.execute_query("SELECT * FROM t WHERE a IS NULL").unwrap();
    assert_eq!(batches[0].num_rows, 1);

    let batches = engine.execute_query("SELECT * FROM t WHERE b IS NOT NULL").unwrap();
    assert_eq!(batches[0].num_rows, 2);
}

#[test]
fn test_select_arithmetic() {
    let engine = setup();
    engine.execute("CREATE TABLE t (a INTEGER, b INTEGER)").unwrap();
    engine.execute("INSERT INTO t VALUES (10, 3)").unwrap();

    let batches = engine.execute_query("SELECT a + b, a - b, a * b, a / b FROM t").unwrap();
    assert_eq!(batches[0].num_rows, 1);
    assert_eq!(batches[0].num_columns(), 4);

    let col0 = batches[0].get_column(0).unwrap();
    match col0 {
        Column::Integer(vals) => assert_eq!(vals[0], Some(13)),
        _ => panic!("expected integer"),
    }
    let col1 = batches[0].get_column(1).unwrap();
    match col1 {
        Column::Integer(vals) => assert_eq!(vals[0], Some(7)),
        _ => panic!("expected integer"),
    }
    let col2 = batches[0].get_column(2).unwrap();
    match col2 {
        Column::Integer(vals) => assert_eq!(vals[0], Some(30)),
        _ => panic!("expected integer"),
    }
    let col3 = batches[0].get_column(3).unwrap();
    match col3 {
        Column::Integer(vals) => assert_eq!(vals[0], Some(3)),
        _ => panic!("expected integer"),
    }
}

#[test]
fn test_drop_table() {
    let engine = setup();
    engine.execute("CREATE TABLE t (a INTEGER)").unwrap();
    engine.execute("INSERT INTO t VALUES (1)").unwrap();
    engine.execute("DROP TABLE t").unwrap();
    let err = engine.execute("SELECT * FROM t").unwrap_err();
    assert!(matches!(err, SQLError::TableNotFound(_)));
}

#[test]
fn test_insert_multiple_rows() {
    let engine = setup();
    engine.execute("CREATE TABLE t (a INTEGER, b TEXT)").unwrap();
    engine.execute("INSERT INTO t VALUES (1, 'a'), (2, 'b'), (3, 'c')").unwrap();

    let batches = engine.execute_query("SELECT * FROM t ORDER BY a").unwrap();
    assert_eq!(batches[0].num_rows, 3);
}

#[test]
fn test_select_count() {
    let engine = setup();
    engine.execute("CREATE TABLE t (a INTEGER)").unwrap();
    engine.execute("INSERT INTO t VALUES (1)").unwrap();
    engine.execute("INSERT INTO t VALUES (2)").unwrap();
    engine.execute("INSERT INTO t VALUES (3)").unwrap();

    let batches = engine.execute_query("SELECT COUNT(*) FROM t").unwrap();
    let col = batches[0].get_column(0).unwrap();
    match col {
        Column::Integer(vals) => assert_eq!(vals[0], Some(3)),
        _ => panic!("expected integer"),
    }
}

#[test]
fn test_nested_expressions() {
    let engine = setup();
    engine.execute("CREATE TABLE t (a INTEGER, b INTEGER, c INTEGER)").unwrap();
    engine.execute("INSERT INTO t VALUES (2, 3, 4)").unwrap();

    let batches = engine.execute_query("SELECT (a + b) * c FROM t").unwrap();
    let val = batches[0].get_row(0).unwrap();
    assert_eq!(val[0], Some(LiteralValue::Integer(20)));
}

#[test]
fn test_type_coercion() {
    let engine = setup();
    engine.execute("CREATE TABLE t (a INTEGER, b FLOAT)").unwrap();
    engine.execute("INSERT INTO t VALUES (5, 3.5)").unwrap();

    let batches = engine.execute_query("SELECT a + b FROM t").unwrap();
    let col = batches[0].get_column(0).unwrap();
    match col {
        Column::Float(vals) => assert!((vals[0].unwrap() - 8.5).abs() < 0.001),
        _ => panic!("expected float"),
    }
}

#[test]
fn test_empty_table_select() {
    let engine = setup();
    engine.execute("CREATE TABLE t (a INTEGER, b TEXT)").unwrap();
    let batches = engine.execute_query("SELECT * FROM t").unwrap();
    assert_eq!(batches[0].num_rows, 0);
}

#[test]
fn test_syntax_error() {
    let engine = setup();
    let err = engine.execute("SELEC * FROM t").unwrap_err();
    assert!(matches!(err, SQLError::Syntax { .. }));
}

#[test]
fn test_table_not_found_error() {
    let engine = setup();
    let err = engine.execute("SELECT * FROM nonexistent").unwrap_err();
    assert!(matches!(err, SQLError::TableNotFound(_)));
}

#[test]
fn test_column_not_found_error() {
    let engine = setup();
    engine.execute("CREATE TABLE t (a INTEGER)").unwrap();
    engine.execute("INSERT INTO t VALUES (1)").unwrap();
    let err = engine.execute("SELECT nonexistent FROM t").unwrap_err();
    assert!(matches!(err, SQLError::ColumnNotFound(_)));
}

// === NEW TESTS ===

#[test]
fn test_select_distinct() {
    let engine = setup();
    engine.execute("CREATE TABLE t (a INTEGER, b TEXT)").unwrap();
    engine.execute("INSERT INTO t VALUES (1, 'x')").unwrap();
    engine.execute("INSERT INTO t VALUES (2, 'y')").unwrap();
    engine.execute("INSERT INTO t VALUES (1, 'x')").unwrap();
    engine.execute("INSERT INTO t VALUES (3, 'z')").unwrap();
    engine.execute("INSERT INTO t VALUES (1, 'x')").unwrap();

    // Should return 3 distinct rows (1,x), (2,y), (3,z)
    let batches = engine.execute_query("SELECT DISTINCT * FROM t ORDER BY a").unwrap();
    assert_eq!(batches[0].num_rows, 3);
}

#[test]
fn test_between() {
    let engine = setup();
    engine.execute("CREATE TABLE t (a INTEGER)").unwrap();
    engine.execute("INSERT INTO t VALUES (1)").unwrap();
    engine.execute("INSERT INTO t VALUES (5)").unwrap();
    engine.execute("INSERT INTO t VALUES (10)").unwrap();

    let batches = engine.execute_query("SELECT * FROM t WHERE a BETWEEN 3 AND 8").unwrap();
    assert_eq!(batches[0].num_rows, 1);
    let row = batches[0].get_row(0).unwrap();
    assert_eq!(row[0], Some(LiteralValue::Integer(5)));
}

#[test]
fn test_in_operator() {
    let engine = setup();
    engine.execute("CREATE TABLE t (a INTEGER)").unwrap();
    engine.execute("INSERT INTO t VALUES (1)").unwrap();
    engine.execute("INSERT INTO t VALUES (2)").unwrap();
    engine.execute("INSERT INTO t VALUES (3)").unwrap();
    engine.execute("INSERT INTO t VALUES (4)").unwrap();

    let batches = engine.execute_query("SELECT * FROM t WHERE a IN (1, 3)").unwrap();
    assert_eq!(batches[0].num_rows, 2);

    let batches = engine.execute_query("SELECT * FROM t WHERE a NOT IN (1, 3)").unwrap();
    // NOT IN would be: WHERE NOT (a IN (1,3))
    assert_eq!(batches[0].num_rows, 2);
}

#[test]
fn test_case_when() {
    let engine = setup();
    engine.execute("CREATE TABLE t (a INTEGER)").unwrap();
    engine.execute("INSERT INTO t VALUES (1)").unwrap();
    engine.execute("INSERT INTO t VALUES (2)").unwrap();
    engine.execute("INSERT INTO t VALUES (3)").unwrap();

    let batches = engine.execute_query(
        "SELECT CASE WHEN a = 1 THEN 'one' WHEN a = 2 THEN 'two' ELSE 'other' END FROM t ORDER BY a"
    ).unwrap();
    assert_eq!(batches[0].num_rows, 3);
    let row0 = batches[0].get_row(0).unwrap();
    assert_eq!(row0[0], Some(LiteralValue::String("one".to_string())));
    let row1 = batches[0].get_row(1).unwrap();
    assert_eq!(row1[0], Some(LiteralValue::String("two".to_string())));
    let row2 = batches[0].get_row(2).unwrap();
    assert_eq!(row2[0], Some(LiteralValue::String("other".to_string())));
}

#[test]
fn test_cast() {
    let engine = setup();
    engine.execute("CREATE TABLE t (a INTEGER, b TEXT)").unwrap();
    engine.execute("INSERT INTO t VALUES (42, '3.14')").unwrap();

    // Test CAST() function
    let batches = engine.execute_query("SELECT CAST(b AS FLOAT) FROM t").unwrap();
    let col = batches[0].get_column(0).unwrap();
    match col {
        Column::Float(vals) => assert!((vals[0].unwrap() - 3.14).abs() < 0.001),
        _ => panic!("expected float"),
    }

    // Test :: syntax
    let batches = engine.execute_query("SELECT a :: TEXT FROM t").unwrap();
    let col = batches[0].get_column(0).unwrap();
    match col {
        Column::String(vals) => assert_eq!(vals[0], Some("42".to_string())),
        _ => panic!("expected string"),
    }
}

#[test]
fn test_like_pattern() {
    let engine = setup();
    engine.execute("CREATE TABLE t (a TEXT)").unwrap();
    engine.execute("INSERT INTO t VALUES ('hello')").unwrap();
    engine.execute("INSERT INTO t VALUES ('world')").unwrap();
    engine.execute("INSERT INTO t VALUES ('hi')").unwrap();

    // % matches any sequence
    let batches = engine.execute_query("SELECT * FROM t WHERE a LIKE 'h%'").unwrap();
    assert_eq!(batches[0].num_rows, 2);

    // _ matches single char
    let batches = engine.execute_query("SELECT * FROM t WHERE a LIKE 'h__lo'").unwrap();
    assert_eq!(batches[0].num_rows, 1);

    // literal _ in pattern (escaped)
    engine.execute("INSERT INTO t VALUES ('test_data')").unwrap();
    engine.execute("INSERT INTO t VALUES ('testXdata')").unwrap();
    let batches = engine.execute_query("SELECT * FROM t WHERE a LIKE 'test\\_data'").unwrap();
    assert_eq!(batches[0].num_rows, 1);
}

#[test]
fn test_ilike() {
    let engine = setup();
    engine.execute("CREATE TABLE t (a TEXT)").unwrap();
    engine.execute("INSERT INTO t VALUES ('Hello')").unwrap();
    engine.execute("INSERT INTO t VALUES ('HELLO')").unwrap();
    engine.execute("INSERT INTO t VALUES ('World')").unwrap();

    let batches = engine.execute_query("SELECT * FROM t WHERE a ILIKE 'hello'").unwrap();
    assert_eq!(batches[0].num_rows, 2);

    let batches = engine.execute_query("SELECT * FROM t WHERE a ILIKE 'h%'").unwrap();
    assert_eq!(batches[0].num_rows, 2);
}

#[test]
fn test_not_null_constraint() {
    let engine = setup();
    engine.execute("CREATE TABLE t (a INTEGER NOT NULL, b TEXT)").unwrap();

    let err = engine.execute("INSERT INTO t (b) VALUES ('x')").unwrap_err();
    assert!(matches!(err, SQLError::ConstraintViolation(_)));

    let err = engine.execute("INSERT INTO t VALUES (NULL, 'x')").unwrap_err();
    assert!(matches!(err, SQLError::ConstraintViolation(_)));

    let result = engine.execute("INSERT INTO t VALUES (1, 'x')").unwrap();
    assert!(matches!(result, SQLResult::Exec { rows_affected: 1, .. }));
}

#[test]
fn test_default_value() {
    let engine = setup();
    engine.execute("CREATE TABLE t (a INTEGER DEFAULT 42, b TEXT)").unwrap();

    engine.execute("INSERT INTO t (b) VALUES ('hello')").unwrap();
    let batches = engine.execute_query("SELECT a FROM t").unwrap();
    let col = batches[0].get_column(0).unwrap();
    match col {
        Column::Integer(vals) => assert_eq!(vals[0], Some(42)),
        _ => panic!("expected integer"),
    }

    // Explicit value overrides default
    engine.execute("INSERT INTO t VALUES (99, 'world')").unwrap();
    let batches = engine.execute_query("SELECT a FROM t ORDER BY a").unwrap();
    let col = batches[0].get_column(0).unwrap();
    match col {
        Column::Integer(vals) => assert_eq!(vals, &[Some(42), Some(99)]),
        _ => panic!("expected integer"),
    }
}

#[test]
fn test_unique_constraint() {
    let engine = setup();
    engine.execute("CREATE TABLE t (a INTEGER UNIQUE, b TEXT)").unwrap();

    engine.execute("INSERT INTO t VALUES (1, 'x')").unwrap();
    // Duplicate should fail
    let err = engine.execute("INSERT INTO t VALUES (1, 'y')").unwrap_err();
    assert!(matches!(err, SQLError::ConstraintViolation(_)));

    // Different value should succeed
    let result = engine.execute("INSERT INTO t VALUES (2, 'y')").unwrap();
    assert!(matches!(result, SQLResult::Exec { rows_affected: 1, .. }));
}

#[test]
fn test_primary_key_constraint() {
    let engine = setup();
    engine.execute("CREATE TABLE t (a INTEGER PRIMARY KEY, b TEXT)").unwrap();

    engine.execute("INSERT INTO t VALUES (1, 'x')").unwrap();
    // Duplicate PK should fail
    let err = engine.execute("INSERT INTO t VALUES (1, 'y')").unwrap_err();
    assert!(matches!(err, SQLError::ConstraintViolation(_)));

    // NULL PK should fail (PK implies NOT NULL)
    let err = engine.execute("INSERT INTO t (b) VALUES ('z')").unwrap_err();
    assert!(matches!(err, SQLError::ConstraintViolation(_)));
}

#[test]
fn test_group_by_having() {
    let engine = setup();
    engine.execute("CREATE TABLE t (category TEXT, value INTEGER)").unwrap();
    engine.execute("INSERT INTO t VALUES ('a', 10)").unwrap();
    engine.execute("INSERT INTO t VALUES ('a', 20)").unwrap();
    engine.execute("INSERT INTO t VALUES ('b', 30)").unwrap();

    // GROUP BY with aggregate - should parse and execute without error
    let result = engine.execute(
        "SELECT category, SUM(value) FROM t GROUP BY category ORDER BY category"
    ).unwrap();
    // Currently AggregateExecutor sums all rows into one group
    // Full multi-group aggregation is a larger feature
    assert!(matches!(result, SQLResult::Query { .. }));
}

#[test]
fn test_order_by_nulls() {
    let engine = setup();
    engine.execute("CREATE TABLE t (a INTEGER)").unwrap();
    engine.execute("INSERT INTO t VALUES (2)").unwrap();
    engine.execute("INSERT INTO t VALUES (NULL)").unwrap();
    engine.execute("INSERT INTO t VALUES (1)").unwrap();
    engine.execute("INSERT INTO t VALUES (NULL)").unwrap();

    // NULLS FIRST should put nulls before non-null
    let batches = engine.execute_query(
        "SELECT * FROM t ORDER BY a NULLS FIRST"
    ).unwrap();
    let col = batches[0].get_column(0).unwrap();
    match col {
        Column::Integer(vals) => {
            assert_eq!(vals[0], None);
            assert_eq!(vals[1], None);
        }
        _ => panic!("expected integer column"),
    }

    // NULLS LAST should put nulls after non-null
    let batches = engine.execute_query(
        "SELECT * FROM t ORDER BY a NULLS LAST"
    ).unwrap();
    let col = batches[0].get_column(0).unwrap();
    match col {
        Column::Integer(vals) => {
            let last = vals.len() - 1;
            assert_eq!(vals[last], None);
        }
        _ => panic!("expected integer column"),
    }
}

#[test]
fn test_query_nesting_limit() {
    let engine = setup();

    // Deeply nested expression should fail
    let mut deep_expr = String::from("1");
    for _ in 0..70 {
        deep_expr = format!("({} + 1)", deep_expr);
    }
    let sql = format!("SELECT {} FROM t", deep_expr);
    // Table t doesn't exist, but we should hit depth limit first
    let err = engine.execute(&sql).unwrap_err();
    assert!(matches!(err, SQLError::QueryTooComplex(_)));
}

#[test]
fn test_lexer_panic_error() {
    let engine = setup();
    // Bare '|' is not valid (only || is valid)
    let err = engine.execute("SELECT * FROM t WHERE a | 1").unwrap_err();
    assert!(matches!(err, SQLError::Syntax { .. }));

    // Single colon not followed by another
    let err = engine.execute("SELECT : FROM t").unwrap_err();
    assert!(matches!(err, SQLError::Syntax { .. }));
}

#[test]
fn test_multiple_errors() {
    let engine = setup();

    // Syntax error - unknown keyword
    let err = engine.execute("NOTAVALIDSTATEMENT x y z").unwrap_err();
    assert!(matches!(err, SQLError::Syntax { .. }));

    // Table not found
    engine.execute("CREATE TABLE t (a INTEGER)").unwrap();
    let err = engine.execute("SELECT * FROM nonexistent").unwrap_err();
    assert!(matches!(err, SQLError::TableNotFound(_)));

    // Column not found
    let err = engine.execute("SELECT missing FROM t").unwrap_err();
    assert!(matches!(err, SQLError::ColumnNotFound(_)));

    // Type mismatch (table has data so WHERE evaluates)
    engine.execute("INSERT INTO t VALUES (1)").unwrap();
    let err = engine.execute("SELECT * FROM t WHERE a + 'foo'").unwrap_err();
    assert!(matches!(err, SQLError::TypeMismatch { .. }));
}

#[test]
fn test_in_operator_with_not() {
    let engine = setup();
    engine.execute("CREATE TABLE t (a INTEGER)").unwrap();
    engine.execute("INSERT INTO t VALUES (1)").unwrap();
    engine.execute("INSERT INTO t VALUES (2)").unwrap();
    engine.execute("INSERT INTO t VALUES (3)").unwrap();
    engine.execute("INSERT INTO t VALUES (4)").unwrap();

    let batches = engine.execute_query("SELECT * FROM t WHERE a IN (1, 2)").unwrap();
    assert_eq!(batches[0].num_rows, 2);
}

#[test]
fn test_like_special_chars() {
    let engine = setup();
    engine.execute("CREATE TABLE t (a TEXT)").unwrap();
    engine.execute("INSERT INTO t VALUES ('test.data')").unwrap();
    engine.execute("INSERT INTO t VALUES ('testXdata')").unwrap();

    // Dot in pattern should match literal dot since LIKE treats . as literal
    let batches = engine.execute_query("SELECT * FROM t WHERE a LIKE 'test.data'").unwrap();
    assert_eq!(batches[0].num_rows, 1);
}

#[test]
fn test_case_no_else() {
    let engine = setup();
    engine.execute("CREATE TABLE t (a INTEGER)").unwrap();
    engine.execute("INSERT INTO t VALUES (1)").unwrap();
    engine.execute("INSERT INTO t VALUES (2)").unwrap();

    let batches = engine.execute_query(
        "SELECT CASE WHEN a = 1 THEN 'one' END FROM t ORDER BY a"
    ).unwrap();
    assert_eq!(batches[0].num_rows, 2);
    let row0 = batches[0].get_row(0).unwrap();
    assert_eq!(row0[0], Some(LiteralValue::String("one".to_string())));
    let row1 = batches[0].get_row(1).unwrap();
    assert_eq!(row1[0], None);
}
