use crate::*;

pub fn parse_crontab(crontab: &String) -> Vec<CronEntry> {
    let mut entries = Vec::new();
    for (line, entry) in crontab.lines().enumerate() {
        let mut entry = entry.trim().to_owned();
        // Remove all comments
        if entry.starts_with("#") || entry.is_empty() {
            //println!("Comment: {}", entry);
            continue;
        }

        // Convert the nonstandard defenitions to normal form
        if entry.starts_with("@") {
            // Reboot is a special case as there is no normal form equivalent
            if entry.split_whitespace().nth(0).unwrap() == "reboot" {
                if let Some(cmd) = entry.split_whitespace().nth(1) {
                    return vec![CronEntry::new_startup_task(cmd)];
                }
                println!(
                    "Parse error: Missing command after predicate, line {}: {}",
                    line, entry
                );
                std::process::exit(1);
            }
            entry = parse_special_expression(&entry, line);
        }

        //println!("{}", entry);

        let mut intervals = Vec::with_capacity(5);
        for (index, expr) in entry.split_whitespace().take(5).enumerate() {
            use CronUnit::*;
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
            minute: CronExpr(CronUnit::Minute, intervals[0].clone()),
            hour: CronExpr(CronUnit::Hour, intervals[1].clone()),
            dom: CronExpr(CronUnit::DayOfMonth, intervals[2].clone()),
            month: CronExpr(CronUnit::Month, intervals[3].clone()),
            dow: CronExpr(CronUnit::DayOfWeek, intervals[4].clone()),
            startup: false,
            cmd: str_vec.join(" "),
        };
        entries.push(e);
    }
    return entries;
}

fn parse_cron_time(unit: &CronUnit, expr: &str, line: usize) -> Option<CronInterval> {
    use CronInterval::*;
    let (min, max) = unit.bounds();

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
            match sub_expr.parse::<u32>() {
                Ok(n) => {
                    let mut points: Vec<u32> = (min..max).filter(|x| x % n == 0).collect();
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
            let start = values[0].parse::<u32>();
            let stop = values[1].parse::<u32>();

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

            let mut points: Vec<u32> = (start..stop + 1).collect();
            time_points.append(&mut points);
        }
        // Parse "single" expression "5"
        else {
            match sub_expr.parse::<u32>() {
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

fn parse_special_expression(expr: &str, line: usize) -> String {
    if let Some(index) = expr.find(|c: char| c.is_whitespace()) {
        let (predicate, cmd) = expr.split_at(index);
        match predicate {
            "@yearly" | "@annually" => return vec!["0 0 1 1 *", cmd].join(" "),
            "@monthly" => return vec!["0 0 1 * *", cmd].join(" "),
            "@weekly" => return vec!["0 0 * * 0", cmd].join(" "),
            "@daily" | "@midnight" => return vec!["0 0 * * *", cmd].join(" "),
            "@hourly" => return vec!["0 * * * *", cmd].join(" "),
            _ => {
                println!(
                    "Parse error: Unknown scheduling: {}, line {}",
                    predicate, line
                );
                std::process::exit(1);
            }
        }
    }

    println!(
        "Parse error: Error parsing command after predicate, line {}: {}",
        line, expr
    );
    std::process::exit(1);
}

fn is_out_of_bounds(x: u32, min: u32, max: u32) -> bool {
    !(x >= min && x <= max)
}