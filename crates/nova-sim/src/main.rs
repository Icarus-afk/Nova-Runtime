mod engine;
mod subsys;

use engine::*;
use subsys::*;
use ratatui::prelude::*;
use ratatui::widgets::*;
use ratatui::layout::{Layout, Direction, Constraint, Rect};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{enable_raw_mode, disable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use std::io::stdout;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

fn level_style(level: &LogLevel) -> Style {
    match level {
        LogLevel::Error => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        LogLevel::Warn => Style::default().fg(Color::Yellow),
        LogLevel::Info => Style::default().fg(Color::Cyan),
        LogLevel::Debug => Style::default().fg(Color::DarkGray),
    }
}

fn level_label(level: &LogLevel) -> &'static str {
    match level {
        LogLevel::Error => "ERROR",
        LogLevel::Warn => "WARN ",
        LogLevel::Info => "INFO ",
        LogLevel::Debug => "DEBUG",
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let mut end = max.saturating_sub(1);
        while !s.is_char_boundary(end) {
            end = end.saturating_sub(1);
        }
        format!("{}…", &s[..end])
    }
}

fn render_header(frame: &mut Frame, area: Rect, eng: &SimEngine) {
    let secs = eng.metrics.uptime_secs.load(Ordering::Relaxed);
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    let uptime = format!("{:02}:{:02}:{:02}", h, m, s);
    let load = eng.load;
    let cpu = eng.metrics.cpu_percent.load(Ordering::Relaxed);
    let mem = eng.metrics.memory_used_mb.load(Ordering::Relaxed);
    let state_label = match eng.state {
        SimStateFlag::Paused => " PAUSED ",
        SimStateFlag::Maintenance => " MAINT ",
        _ => " RUNNING ",
    };
    let state_style = match eng.state {
        SimStateFlag::Paused | SimStateFlag::Maintenance => Style::default().fg(Color::Yellow).bg(Color::Blue),
        _ => Style::default().fg(Color::Green).bg(Color::Black),
    };

    let text = Line::from(vec![
        Span::styled(" Nova Runtime ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled(state_label, state_style),
        Span::raw(format!(" Up:{uptime} Load:{load}% CPU:{cpu}% Mem:{mem}MB Req:{}", eng.metrics.requests_total.load(Ordering::Relaxed))),
    ]);
    let block = Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::DarkGray));
    let p = Paragraph::new(text).block(block).alignment(Alignment::Left);
    frame.render_widget(p, area);
}

fn render_logs(frame: &mut Frame, area: Rect, eng: &SimEngine) {
    let inner_w = (area.width as usize).saturating_sub(2);
    let max_rows = (area.height as usize).saturating_sub(2);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Logs ")
        .title_alignment(Alignment::Left);

    let lines: Vec<Line> = eng.logs.iter().rev().take(max_rows).map(|entry| {
        let ts = entry.timestamp.format("%H:%M:%S%.3f").to_string();
        let lvl = level_label(&entry.level);
        let sub = format!("[{}]", entry.subsystem);
        let prefix = format!("{} {} {} ", ts, lvl, sub);
        let prefix_len = prefix.len();

        let msg_text = &entry.message;
        let suffix = match (&entry.request_id, entry.duration_ms) {
            (Some(id), Some(d)) => format!(" ({id}) [{}ms]", d),
            (Some(id), None) => format!(" ({id})"),
            (None, Some(d)) => format!(" [{}ms]", d),
            (None, None) => String::new(),
        };
        let msg_max = inner_w.saturating_sub(prefix_len);
        let msg_trunc = truncate(msg_text, msg_max);

        Line::from(vec![
            Span::styled(ts, Style::default().fg(Color::DarkGray)),
            Span::raw(" "),
            Span::styled(lvl, level_style(&entry.level)),
            Span::raw(" "),
            Span::styled(sub, Style::default().fg(Color::Rgb(120, 180, 240))),
            Span::raw(" "),
            Span::styled(msg_trunc, Style::default().fg(match entry.level {
                LogLevel::Error => Color::Red,
                LogLevel::Warn => Color::Yellow,
                _ => Color::White,
            })),
            Span::styled(suffix, Style::default().fg(Color::DarkGray)),
        ])
    }).collect();

    let p = Paragraph::new(lines).block(block);
    frame.render_widget(p, area);
}

fn render_metrics(frame: &mut Frame, area: Rect, eng: &SimEngine) {
    let inner_w = (area.width as usize).saturating_sub(2);
    let _max_rows = (area.height as usize).saturating_sub(2);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Metrics ")
        .title_alignment(Alignment::Left);

    let rps = eng.metrics.requests_total.load(Ordering::Relaxed);
    let auth_ok = eng.metrics.auth_success.load(Ordering::Relaxed);
    let auth_fail = eng.metrics.auth_failure.load(Ordering::Relaxed);
    let ch = eng.metrics.cache_hits.load(Ordering::Relaxed);
    let cm = eng.metrics.cache_misses.load(Ordering::Relaxed);
    let ttl = ch + cm;
    let hr = if ttl > 0 { ch * 100 / ttl } else { 100 };
    let qd = eng.metrics.queue_depth.load(Ordering::Relaxed);
    let qpub = eng.metrics.queue_published.load(Ordering::Relaxed);
    let qcon = eng.metrics.queue_consumed.load(Ordering::Relaxed);
    let sf = eng.metrics.scheduler_jobs_fired.load(Ordering::Relaxed);
    let sa = eng.metrics.scheduler_jobs_active.load(Ordering::Relaxed);
    let sr = eng.metrics.storage_reads.load(Ordering::Relaxed);
    let sw = eng.metrics.storage_writes.load(Ordering::Relaxed);
    let ep = eng.metrics.events_published.load(Ordering::Relaxed);
    let wa = eng.metrics.workers_active.load(Ordering::Relaxed);
    let wi = eng.metrics.workers_idle.load(Ordering::Relaxed);
    let cpu = eng.metrics.cpu_percent.load(Ordering::Relaxed);
    let mu = eng.metrics.memory_used_mb.load(Ordering::Relaxed);
    let mt = eng.metrics.memory_total_mb.load(Ordering::Relaxed);
    let secs = eng.metrics.uptime_secs.load(Ordering::Relaxed);
    let rate = if secs > 0 { rps / secs } else { 0 };

    let items: Vec<String> = vec![
        format!("Req: {rps}  ({rate}/s)"),
        format!("Auth OK: {auth_ok}  Fail: {auth_fail}"),
        format!("Cache: {ch}h {cm}m {hr}%"),
        format!("Queue: {qd}d  P:{qpub} C:{qcon}"),
        format!("Sched: {sf} fired  {sa} act"),
        format!("Stor: R{sr} W{sw}"),
        format!("Evts: {ep}"),
        format!("Work: {wa} act  {wi} idle"),
        format!("CPU: {cpu}%  Mem: {mu}/{mt}MB"),
    ];

    let lines: Vec<Line> = items.into_iter().map(|l| Line::from(Span::raw(truncate(&l, inner_w)))).collect();
    let p = Paragraph::new(lines).block(block);
    frame.render_widget(p, area);
}

fn render_status_bar(frame: &mut Frame, area: Rect, eng: &SimEngine) {
    let paused = eng.state == SimStateFlag::Paused;
    let maint = eng.state == SimStateFlag::Maintenance;
    let fail = eng.failure_injected;
    let verbose = eng.verbose;
    let speed = eng.clock.speed();

    let mut left = String::from(" Nova-Sim ");
    if paused { left.push_str(" ⏸ PAUSED "); }
    if maint { left.push_str(" 🛠 MAINT "); }
    if fail { left.push_str(" ⚠ FAILURE "); }
    if verbose { left.push_str(" VERBOSE "); }
    let _right = format!(" Speed: {speed:.1}x ");

    let block = Block::default().borders(Borders::TOP).border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    frame.render_widget(Paragraph::new(Line::from(Span::raw(left))), inner);
}

fn render_controls_hint(frame: &mut Frame, area: Rect) {
    let text = "[Space] Pause  [+/-] Load  [s] Cycle speed  [/] Slower  [*] Faster  [f] Failure  [m] Maint  [v] Verbose  [c] Clear  [q] Quit";
    let block = Block::default().borders(Borders::TOP).border_style(Style::default().fg(Color::DarkGray));
    let p = Paragraph::new(Line::from(Span::styled(text, Style::default().fg(Color::Cyan))))
        .block(block).alignment(Alignment::Center);
    frame.render_widget(p, area);
}

fn run_headless(mut eng: SimEngine, ticks: u64, output: &str) -> anyhow::Result<()> {
    let tick_rate = Duration::from_millis(eng.config.tick_rate_ms);
    let start = Instant::now();
    for _ in 0..ticks {
        std::thread::sleep(tick_rate);
        eng.tick();
    }
    let wall_secs = start.elapsed().as_secs_f64();

    let m = &eng.metrics;
    let rps = m.requests_total.load(Ordering::Relaxed);
    let h2 = m.http_2xx.load(Ordering::Relaxed);
    let h4 = m.http_4xx.load(Ordering::Relaxed);
    let h5 = m.http_5xx.load(Ordering::Relaxed);
    let logs: Vec<serde_json::Value> = eng.logs.iter().map(|e| serde_json::json!({
        "ts": e.timestamp.to_rfc3339(),
        "level": format!("{:?}", e.level),
        "subsystem": e.subsystem,
        "message": e.message,
        "request_id": e.request_id,
        "duration_ms": e.duration_ms,
    })).collect();

    let result = serde_json::json!({
        "summary": {
            "wall_clock_secs": wall_secs,
            "virtual_uptime_secs": m.uptime_secs.load(Ordering::Relaxed),
            "ticks": ticks,
            "requests_total": rps,
            "requests_per_sec": if wall_secs > 0.0 { (rps as f64 / wall_secs * 100.0).round() / 100.0 } else { 0.0 },
            "http_2xx": h2,
            "http_4xx": h4,
            "http_5xx": h5,
            "http_success_rate": if rps > 0 { (h2 as f64 / rps as f64 * 10000.0).round() / 100.0 } else { 0.0 },
            "auth_success": m.auth_success.load(Ordering::Relaxed),
            "auth_failure": m.auth_failure.load(Ordering::Relaxed),
            "cache_hits": m.cache_hits.load(Ordering::Relaxed),
            "cache_misses": m.cache_misses.load(Ordering::Relaxed),
            "sql_queries": m.sql_queries.load(Ordering::Relaxed),
            "sql_slow": m.sql_slow.load(Ordering::Relaxed),
            "queue_published": m.queue_published.load(Ordering::Relaxed),
            "queue_consumed": m.queue_consumed.load(Ordering::Relaxed),
            "scheduler_jobs_fired": m.scheduler_jobs_fired.load(Ordering::Relaxed),
            "search_indexed": m.search_indexed.load(Ordering::Relaxed),
            "search_queries": m.search_queries.load(Ordering::Relaxed),
            "blob_uploads": m.blob_uploads.load(Ordering::Relaxed),
            "blob_downloads": m.blob_downloads.load(Ordering::Relaxed),
            "storage_reads": m.storage_reads.load(Ordering::Relaxed),
            "storage_writes": m.storage_writes.load(Ordering::Relaxed),
            "events_published": m.events_published.load(Ordering::Relaxed),
            "cpu_percent": m.cpu_percent.load(Ordering::Relaxed),
            "memory_used_mb": m.memory_used_mb.load(Ordering::Relaxed),
        },
        "logs": logs,
    });

    let json = serde_json::to_string_pretty(&result)?;
    std::fs::write(output, &json)?;

    println!("═══ Nova Sim — Headless Results ═══");
    println!("  Duration:  {wall_secs:.1}s wall  ({} virtual)", m.uptime_secs.load(Ordering::Relaxed));
    println!("  Ticks:     {ticks}");
    println!("  Requests:  {rps} total  ({:.1}/s)", rps as f64 / wall_secs);
    println!("  HTTP 2xx:  {h2}  4xx: {h4}  5xx: {h5}");
    println!("  Success:   {:.1}%", if rps > 0 { h2 as f64 / rps as f64 * 100.0 } else { 0.0 });
    println!("  Cache:     {} hits / {} misses", m.cache_hits.load(Ordering::Relaxed), m.cache_misses.load(Ordering::Relaxed));
    println!("  Auth:      {} ok / {} fail", m.auth_success.load(Ordering::Relaxed), m.auth_failure.load(Ordering::Relaxed));
    println!("  Queue:     {} pub / {} con", m.queue_published.load(Ordering::Relaxed), m.queue_consumed.load(Ordering::Relaxed));
    println!("  Scheduler: {} fired", m.scheduler_jobs_fired.load(Ordering::Relaxed));
    println!("  Logs:      {} entries written to {output}", eng.logs.len());
    println!("═══════════════════════════════════");
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut api_target = "http://127.0.0.1:8642".to_string();
    let mut headless = false;
    let mut ticks: u64 = 100;
    let mut output = "sim-results.json".to_string();

    {
        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--headless" => headless = true,
                "--ticks" if i + 1 < args.len() => { i += 1; ticks = args[i].parse().unwrap_or(100); }
                "--output" if i + 1 < args.len() => { i += 1; output = args[i].clone(); }
                a if !a.starts_with("--") => api_target = a.to_string(),
                _ => {}
            }
            i += 1;
        }
    }

    let config = SimConfig::default();
    let mut eng = SimEngine::new(config);

    eng.register(Box::new(WorkerSubsystem::new()));
    eng.register(Box::new(HttpSubsystem::new(&api_target)));
    eng.register(Box::new(AuthSubsystem::new()));
    eng.register(Box::new(SqlSubsystem::new()));
    eng.register(Box::new(CacheSubsystem::new()));
    eng.register(Box::new(QueueSubsystem::new()));
    eng.register(Box::new(SchedulerSubsystem::new()));
    eng.register(Box::new(SearchSubsystem::new()));
    eng.register(Box::new(BlobSubsystem::new()));
    eng.register(Box::new(StorageSubsystem::new()));
    eng.register(Box::new(EventBusSubsystem::new()));

    eng.log(LogLevel::Info, "system", "Nova Runtime Simulation v0.1.0 starting...".into());
    eng.log(LogLevel::Info, "system", format!("Seed: {} | Tick rate: {}ms", 42, eng.config.tick_rate_ms));
    eng.log(LogLevel::Info, "system", format!("Target API: {api_target}").into());
    eng.log(LogLevel::Info, "system", "Registered 11 subsystems".into());

    if headless {
        eng.verbose = true;
        return run_headless(eng, ticks, &output);
    }

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    eng.log(LogLevel::Info, "system", "Initialization complete — entering run loop".into());

    let mut last_render = Instant::now();
    let tick_rate = Duration::from_millis(eng.config.tick_rate_ms);

    loop {
        let now = Instant::now();
        if now.duration_since(last_render) >= tick_rate {
            eng.tick();
            terminal.draw(|f| {
                let size = f.area();
                let vert = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(1),
                        Constraint::Min(10),
                        Constraint::Length(1),
                        Constraint::Length(1),
                    ])
                    .split(size);

                render_header(f, vert[0], &eng);

                let mids = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                    .split(vert[1]);
                render_logs(f, mids[0], &eng);
                render_metrics(f, mids[1], &eng);

                render_status_bar(f, vert[2], &eng);
                render_controls_hint(f, vert[3]);
            })?;
            last_render = now;
        }

        if event::poll(Duration::from_millis(10))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char(' ') => {
                            eng.state = match eng.state {
                                SimStateFlag::Running => {
                                    eng.log(LogLevel::Info, "system", "Simulation paused".into());
                                    SimStateFlag::Paused
                                }
                                _ => {
                                    eng.log(LogLevel::Info, "system", "Simulation resumed".into());
                                    SimStateFlag::Running
                                }
                            };
                        }
                        KeyCode::Char('+') | KeyCode::Char('=') => {
                            eng.load = (eng.load + 10).min(100);
                            eng.log(LogLevel::Info, "system", format!("Load increased to {}%", eng.load));
                        }
                        KeyCode::Char('-') | KeyCode::Char('_') => {
                            eng.load = eng.load.saturating_sub(10).max(5);
                            eng.log(LogLevel::Info, "system", format!("Load decreased to {}%", eng.load));
                        }
                        KeyCode::Char('s') => {
                            eng.clock.cycle_speed();
                            eng.log(LogLevel::Info, "system", format!("Speed: {:.2}x", eng.clock.speed()));
                        }
                        KeyCode::Char('/') => {
                            let s = (eng.clock.speed() * 0.5).max(0.125);
                            eng.clock.set_speed(s);
                            eng.log(LogLevel::Info, "system", format!("Speed: {:.2}x", eng.clock.speed()));
                        }
                        KeyCode::Char('*') => {
                            let s = (eng.clock.speed() * 2.0).min(8.0);
                            eng.clock.set_speed(s);
                            eng.log(LogLevel::Info, "system", format!("Speed: {:.2}x", eng.clock.speed()));
                        }
                        KeyCode::Char('f') => {
                            if eng.failure_injected { eng.clear_failure(); } else { eng.inject_failure(); }
                        }
                        KeyCode::Char('m') => {
                            eng.state = match eng.state {
                                SimStateFlag::Maintenance => {
                                    eng.log(LogLevel::Info, "system", "Maintenance mode deactivated".into());
                                    SimStateFlag::Running
                                }
                                _ => {
                                    eng.log(LogLevel::Info, "system", "Maintenance mode activated".into());
                                    SimStateFlag::Maintenance
                                }
                            };
                        }
                        KeyCode::Char('v') => {
                            eng.verbose = !eng.verbose;
                            eng.log(LogLevel::Info, "system", if eng.verbose { "Verbose logging enabled" } else { "Verbose logging disabled" }.into());
                        }
                        KeyCode::Char('c') => {
                            eng.logs = LogBuffer::new(eng.config.log_capacity);
                        }
                        _ => {}
                    }
                }
                Event::Resize(_, _) => { terminal.autoresize()?; }
                _ => {}
            }
        }
    }

    eng.log(LogLevel::Info, "system", "Shutting down gracefully...".into());
    let _ = terminal.draw(|f| {
        let size = f.area();
        render_header(f, size, &eng);
    });

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    println!("Nova Runtime Simulation stopped.");
    println!("  Simulated uptime: {}s", eng.metrics.uptime_secs.load(Ordering::Relaxed));
    println!("  Requests handled: {}", eng.metrics.requests_total.load(Ordering::Relaxed));
    println!("  Events published: {}", eng.metrics.events_published.load(Ordering::Relaxed));
    Ok(())
}
