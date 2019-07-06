use crate::*;

pub fn parse_crontab(crontab: &String) -> Vec<CronEntry> {
    let mut entries = Vec::new();
    for (line, entry) in crontab.lines().enumerate() {
        // Remove all comments
        if entry.starts_with("#") {
            //println!("Comment: {}", entry);
            continue;
        }

        // println!("{}", entry);

        let mut intervals = Vec::with_capacity(5);
        for (index, expr) in entry.split_whitespace().take(5).enumerate() {
            use CronExprKind::*;
            let cron_expr_kind = match index {
                0 => Minute,
                1 => Hour,
                2 => DayOfMonth,
                3 => Month,
                4 => DayOfWeek,
                _ => unreachable!(),
            };

            if let Some(interval) = parse_cron_time(&cron_expr_kind, expr, line) {
                intervals.push(interval)
            } else {
                println!(
                    "Parse error: Could not match any expression at line {}",
                    line + 1
                );
                std::process::exit(1);
            }
        }
        let str_vec: Vec<&str> = entry.split_whitespace().skip(5).collect();

        let e = CronEntry {
            minute: intervals[0].clone(),
            hour: intervals[1].clone(),
            dom: intervals[2].clone(),
            month: intervals[3].clone(),
            dow: intervals[4].clone(),
            cmd: str_vec.join(" "),
        };
        entries.push(e);
    }
    return entries;
}

fn parse_cron_time(kind: &CronExprKind, expr: &str, line: usize) -> Option<CronInterval> {
    use CronInterval::*;
    let (min, max) = kind.bounds();

    //println!("{}", expr);
    // Handle comma-separated expressions "15,5-10"
    let mut time_points = Vec::new();
    for sub_expr in expr.split(",") {
        // N.B. If any sub expression is * then we can just return Every
        if sub_expr == "*" {
            return Some(Every);
        }
        // Parse "fraction" expressions "*/5"
        else if sub_expr.starts_with("*/") {
            let sub_expr = sub_expr.replace("*/", "");
            match sub_expr.parse::<u8>() {
                Ok(n) => {
                    let mut points: Vec<u8> = (min..max).filter(|x| x % n == 0).collect();
                    time_points.append(&mut points);
                }
                Err(..) => {
                    println!(
                        "Parse error: Expression is not an integer, line {}: {}",
                        line, sub_expr
                    );
                    std::process::exit(1);
                }
            };
        }
        // Parse "range" expression "30-45"
        else if sub_expr.contains("-") {
            let values: Vec<&str> = sub_expr.split("-").take(2).collect();
            // TODO handle panics here
            let start = values[0].parse::<u8>();
            let stop = values[1].parse::<u8>();

            if start.is_err() || stop.is_err() {
                println!(
                    "Parse error: Expression is not an integer, line {}: {}",
                    line, sub_expr
                );
                std::process::exit(1);
            }

            let start = start.unwrap();
            let stop = stop.unwrap();

            if is_out_of_bounds(start, min, max) {
                println!(
                    "Parse error: Value out of bounds, should be between {} and {}. line {}: {}",
                    min, max, line, start
                );
                std::process::exit(1);
            }
            if is_out_of_bounds(stop, min, max) {
                println!(
                    "Parse error: Value out of bounds, should be between {} and {}. line {}: {}",
                    min, max, line, start
                );
                std::process::exit(1);
            }

            if start == stop {
                println!(
                    "Parse error: Start and stop should not be the same value, line {}: {}",
                    line, sub_expr
                );
                std::process::exit(1);
            }

            if start > stop {
                println!(
                    "Parse error: Start should not be before than stop, line {}: {}",
                    line, sub_expr
                );
                std::process::exit(1);
            }

            if stop < start {
                println!(
                    "Parse error: Stop should not be after than start, line {}: {}",
                    line, sub_expr
                );
                std::process::exit(1);
            }

            let mut points: Vec<u8> = (start..stop + 1).collect();
            time_points.append(&mut points);
        }
        // Parse "single" expression "5"
        else {
            match sub_expr.parse::<u8>() {
                Ok(n) if n >= min && n <= max => {
                    time_points.push(n);
                }
                Ok(n) => {
                    println!(
                      "Parse error: Value out of bounds, should be between {} and {}. line {}: {}",
                      min, max, line, n
                  );
                    std::process::exit(1);
                }
                Err(..) => {
                    println!(
                        "Parse error: Expression is not an integer, line {}: {}",
                        line, sub_expr
                    );
                    std::process::exit(1);
                }
            };
        }
    }

    if !time_points.is_empty() {
        time_points.sort();
        time_points.dedup();
        return Some(Multiple(time_points));
    }

    return None;
}

fn is_out_of_bounds(x: u8, min: u8, max: u8) -> bool {
    !(x >= min && x <= max)
}