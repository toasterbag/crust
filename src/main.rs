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

    pub fn next_attempt(&self, now: DateTime<Local>) -> DateTime<Local> {
        //println!("{}", now);

        let date = self.next_date(now.date());
        let time = self.next_time(now);
        //println!("{}", date);
        //println!("{}", time);

        return date.and_hms_nano(time.hour(), time.minute(), 0, 0);
    }

    fn is_hourly(&self) -> bool {
        self.minute.is_multiple() && self.hour.is_every()
    }

    fn date_matters(&self) -> bool {
        (self.minute.is_every() || self.hour.is_every())
            && (self.month.is_multiple() || self.dom.is_multiple() || self.dow.is_multiple())
    }

    fn next_date(&self, now: Date<Local>) -> Date<Local> {
        if !self.month.contains(now.month()) {
            let next_month = self.month.next_from(now.month());
            if next_month <= now.month() {
                return self.next_date(
                    now.with_year(now.year() + 1)
                        .unwrap()
                        .with_month(next_month)
                        .unwrap()
                        .with_day(1)
                        .unwrap(),
                );
            }
            return self.next_date(now.with_month(next_month).unwrap().with_day(1).unwrap());
        }

        if !self.dom.contains(now.day()) {
            let next_day = self.dom.next_from(now.day());
            if next_day > now.day() && next_day <= 28 {
                return self.next_date(now.with_day(next_day).unwrap());
            }
            return self.next_date(now + Duration::days(1));
        }

        /*
        let mut future = now.date();
        let weekday = future.weekday().num_days_from_sunday();
        if !self.dow.contains(weekday) {
            let delta = self.dow.next_from(weekday);
            let delta = Duration::days((weekday - 6 + delta) as i64);
            future = future + delta;
        }*/

        if now <= Local::now().date() && self.date_matters() {
            return self.next_date(now + Duration::days(1));
        }

        return now;
    }

    fn next_hour(&self, now: u32) -> u32 {
        if !self.hour.contains(now) {
            let next_hour = self.hour.next_from(now);
            if next_hour <= now {
                return self.next_hour((now + 1) % 24);
            }
            return next_hour;
        } else if self.is_hourly() {
            return (now + 1) % 24;
        }
        return now;
    }

    fn next_time(&self, now: DateTime<Local>) -> DateTime<Local> {
        let hour = self.next_hour(now.hour());

        let mut now = now.with_hour(hour).unwrap();

        let mut minute = now.minute();
        while !self.minute.contains(minute) {
            let next_minute = self.minute.next_from(now.minute());
            if next_minute <= minute {
                now = now + Duration::hours(1);
                minute = next_minute;
            }
            minute = minute + 1;
        }

        return now.with_minute(minute).unwrap();
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
    thread::spawn(move || {
        if entry.startup {
            let output = Command::new("/bin/sh")
                .arg("-c")
                .arg(&entry.cmd)
                .output()
                .expect("failed to execute process");
            //println!("{}", String::from_utf8(output.stdout).unwrap());
            //eprintln!("{}", String::from_utf8(output.stderr).unwrap());
            //break;
        }
        let future = entry.next_attempt(Local::now() + Duration::minutes(1));
        println!("Scheduling: `{}` for {}", entry.cmd, future);
        let t = future - Local::now();
        //sleep(t.to_std().unwrap());

        let output = Command::new("/bin/sh")
            .arg("-c")
            .arg(&entry.cmd)
            .output()
            .expect("failed to execute process");
    })
}
