#[macro_use(quickcheck)]

use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use std::process::Command;
use std::thread;

use std::thread::sleep;
use std::thread::JoinHandle;

use chrono::prelude::*;
use chrono::Duration;
use clap::{App, Arg};

mod parser;
use parser::parse_crontab;

mod expr;
use expr::*;

struct Args {
    pub config_path: String,
}

#[derive(Clone, Debug)]
pub struct CronEntry {
    minute: CronExpr,
    hour: CronExpr,
    dom: CronExpr,
    month: CronExpr,
    dow: CronExpr,
    startup: bool,
    cmd: String,
}

fn is_leap_year(year: i32) -> bool {
    ((year % 4 == 0) && (year % 100 != 0)) || (year % 400 == 0)
}

fn month_length(date: &DateTime<Local>) -> u32 {
    match date.month() {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => match is_leap_year(date.year()) {
            true => 29,
            false => 28,
        },
        _ => unimplemented!(),
    }
}

fn month_has_day(date: &DateTime<Local>, day: u32) -> bool {
    month_length(date) >= day
}

impl CronEntry {
    pub fn new_startup_task(cmd: &str) -> CronEntry {
        CronEntry {
            minute: CronExpr(CronUnit::Minute, CronInterval::Every),
            hour: CronExpr(CronUnit::Hour, CronInterval::Every),
            dom: CronExpr(CronUnit::DayOfMonth, CronInterval::Every),
            month: CronExpr(CronUnit::Month, CronInterval::Every),
            dow: CronExpr(CronUnit::DayOfWeek, CronInterval::Every),
            startup: true,
            cmd: cmd.to_owned(),
        }
    }

    pub fn next_execution(&self, mut now: DateTime<Local>) -> DateTime<Local> {
        loop {
            if self.minute.is_multiple() && !self.minute.contains(now.minute()) {
                let minute = self.minute.next_from(now.minute());
                if minute < now.minute() {
                    now = now + Duration::hours(1);
                }
                now = now.with_minute(minute).unwrap();
            }

            if self.hour.is_multiple() && !self.minute.contains(now.minute()) {
                let hour = self.hour.next_from(now.hour());
                if hour < now.hour() {
                    now = now.with_hour(hour).unwrap().with_minute(0).unwrap();
                    now = now + Duration::days(1);
                    continue;
                }
                now = now.with_hour(hour).unwrap().with_minute(0).unwrap();
                continue;
            }

            let weekday = now.weekday().num_days_from_sunday();
            if self.dow.is_multiple() && !self.dow.contains(weekday) {
                let delta = (self.dow.next_from(weekday) as i64 - weekday as i64 + 7) % 7;
                now = now + Duration::days(delta);
                now = now.with_hour(0).unwrap().with_minute(0).unwrap();
                continue;
            }

            let next_month = now.month() % 12 + 1;
            if self.dom.is_multiple() && !self.dom.contains(now.day()) {
                let day = self.dom.next_from(now.day());
                if day < now.day() || month_has_day(&now.with_month(next_month).unwrap(), day) {
                    let len = month_length(&now);
                    let days_left_in_month = len - now.day() + 1;
                    now = now + Duration::days(days_left_in_month as i64);
                    now = now
                        .with_day(1)
                        .unwrap()
                        .with_hour(0)
                        .unwrap()
                        .with_minute(0)
                        .unwrap();
                    continue;
                }
                now = now
                    .with_day(day)
                    .unwrap()
                    .with_hour(0)
                    .unwrap()
                    .with_minute(0)
                    .unwrap();
                continue;
            }

            if self.month.is_multiple() && !self.month.contains(now.month()) {
                let month = self.month.next_from(now.month());
                if month < now.month() {
                    let months_to_add = 12 - now.month() + month;
                    let next_month = now.month() + months_to_add;
                    if next_month > 12 {
                        now.with_year(now.year() + 1)
                            .unwrap()
                            .with_month(next_month % 13 + 1)
                            .unwrap()
                            .with_day(1)
                            .unwrap()
                            .with_hour(0)
                            .unwrap()
                            .with_minute(0)
                            .unwrap();
                        continue;
                    }
                    now.with_month(next_month)
                        .unwrap()
                        .with_day(1)
                        .unwrap()
                        .with_hour(0)
                        .unwrap()
                        .with_minute(0)
                        .unwrap();
                    continue;
                }
                now.with_month(month)
                    .unwrap()
                    .with_day(1)
                    .unwrap()
                    .with_hour(0)
                    .unwrap()
                    .with_minute(0)
                    .unwrap();
                continue;
            }
            break;
        }

        return now.with_second(0).unwrap().with_nanosecond(0).unwrap();
    }

    fn is_hourly(&self) -> bool {
        self.minute.is_multiple() && self.hour.is_every()
    }

    fn date_matters(&self) -> bool {
        (self.minute.is_every() || self.hour.is_every())
            && (self.month.is_multiple() || self.dom.is_multiple() || self.dow.is_multiple())
    }
}

fn main() {
    let args = gen_args();
    if let Ok(crontab) = read_crontab(&args.config_path) {
        let crontab = parse_crontab(&crontab);

        let mut handles = Vec::new();
        for entry in crontab {
            handles.push(spawn_entry(&entry));
            sleep(Duration::milliseconds(100).to_std().unwrap());
        }

        for child in handles {
            match child.join() {
                Ok(_) => continue,
                Err(e) => continue, //println!("{:?}", e),
            }
        }

    }
}

fn gen_args() -> Args {
    let matches = App::new("Crust")
        .version("0.1.0")
        .author("Karl David Hedgren. <david@davebay.net>")
        .about("rust + cron = crust!")
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .help("Sets crontab path")
                .takes_value(true)
                .default_value("$HOME/.config/crontab"),
        )
        .get_matches();

    let config_path = matches.value_of("config").unwrap();
    let home = std::env::var("HOME").unwrap_or(String::from("/"));
    let config_path = config_path.replace("$HOME", &home);

    if !Path::new(&config_path).exists() {
        println!("Could not find crontab: {}", config_path);
        std::process::exit(1);
    }

    Args {
        config_path: String::from(config_path),
    }

}

fn read_crontab(path: &String) -> std::io::Result<String> {
    let mut crontab_file = File::open(&path)?;
    let mut crontab_string = String::new();
    crontab_file.read_to_string(&mut crontab_string)?;
    return Ok(crontab_string);
}

fn spawn_entry(entry: &CronEntry) -> JoinHandle<()> {
    let entry = (*entry).clone();
    thread::spawn(move || loop {
        if entry.startup {
            let output = Command::new("/bin/sh")
                .arg("-c")
                .arg(&entry.cmd)
                .output()
                .expect("failed to execute process");
            println!("{}", String::from_utf8(output.stdout).unwrap());
            eprintln!("{}", String::from_utf8(output.stderr).unwrap());
            break;
        }
        let future = entry.next_execution(Local::now() + Duration::minutes(1));
        println!("Scheduling: `{}` for {}", entry.cmd, future);
        let t = future - Local::now();
        sleep(t.to_std().unwrap());

        let output = Command::new("/bin/sh")
            .arg("-c")
            .arg(&entry.cmd)
            .output()
            .expect("failed to execute process");
        println!("{}", String::from_utf8(output.stdout).unwrap());
        eprintln!("{}", String::from_utf8(output.stderr).unwrap());
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn leap_year() {
        assert!(is_leap_year(1804));
        assert!(is_leap_year(1904));
        assert!(is_leap_year(2004));
        assert!(is_leap_year(2012));
        assert!(is_leap_year(2016));
        assert!(is_leap_year(2020));
        assert!(is_leap_year(2316));

        assert!(!is_leap_year(1823));
        assert!(!is_leap_year(1825));
        assert!(!is_leap_year(2001));
        assert!(!is_leap_year(2002));
        assert!(!is_leap_year(1999));
        assert!(!is_leap_year(2000));
    }
}