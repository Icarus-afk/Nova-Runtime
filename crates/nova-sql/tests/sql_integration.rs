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
    assert!(matches!(err, SQLError::Syntax(_)));
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
