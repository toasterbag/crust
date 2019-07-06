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

struct Args {
    pub config_path: String,
}

#[derive(Debug)]
pub enum CronUnit {
    Minute,
    Hour,
    DayOfMonth,
    Month,
    DayOfWeek,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CronInterval {
    Every,
    Multiple(Vec<u32>),
}

pub struct CronExpr (CronUnit, CronInterval);

impl CronUnit {
    pub fn bounds(&self) -> (u32, u32) {
        use CronUnit::*;
        match self {
            Minute => (0, 59),
            Hour => (0, 23),
            DayOfMonth => (1, 31),
            Month => (1, 12),
            DayOfWeek => (0, 6),
        }
    }
}

impl CronInterval {
    pub fn last(&self, kind: CronUnit) -> u32 {
        let (min, max) = kind.bounds();
        match self {
            CronInterval::Every => max,
            CronInterval::Multiple(v) => *v.iter().rev().nth(0).unwrap(),
        }
    }

    pub fn distance(interval: &CronInterval, current: u32, kind: CronUnit) -> u32 {
        let (min, max) = kind.bounds();
        match interval {
            CronInterval::Every => 0,
            CronInterval::Multiple(v) => {
                let remainder: Vec<u32> = v
                    .iter()
                    .filter(|x| **x > current)
                    .map(|x| x.clone())
                    .collect();
                if !remainder.is_empty() {
                    return *remainder.iter().nth(0).unwrap();
                }

                // Kill me
                return ((current as i32 - max as i32) as i32 + (*v.iter().nth(0).unwrap()) as i32)
                    as u32;
            }
        }
    }

    pub fn closest(interval: &CronInterval, current: u32, kind: CronUnit) -> Option<u32> {
        let (min, max) = kind.bounds();
        match interval {
            CronInterval::Every => None,
            CronInterval::Multiple(v) => {
                let remainder = v.iter().filter(|x| **x > current).map(|x| x.clone()).nth(0);
                if remainder.is_some() {
                    return remainder;
                }

                return Some(v.iter().nth(0).unwrap().clone());
            }
        }
    }
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

    pub fn next_attempt(&self) -> usize {
        let now = Local::now();

        let correct_month = match &self.month {
            CronInterval::Every => true,
            CronInterval::Multiple(v) => v.contains(&now.month()),
        };

        if !correct_month {
            let mut future = now.clone();
            if now.month() > self.month.last().unwrap() {

            }
        }

        return (future - now).num_seconds() as usize;
    }

    pub fn is_correct_day(&self) -> bool {
        let now = Local::now();

        let correct_month = match &self.month {
            CronInterval::Every => true,
            CronInterval::Multiple(v) => v.contains(&(now.month())),
        };

        let correct_dow = match &self.dow {
            CronInterval::Every => true,
            CronInterval::Multiple(v) => v.contains(&(now.weekday().num_days_from_sunday())),
        };

        let correct_dom = match &self.dom {
            CronInterval::Every => true,
            CronInterval::Multiple(v) => v.contains(&(now.day())),
        };

        return correct_month && correct_dow && correct_dom;
    }

    fn next_day(target: u32) {}
}

fn main() {
    let args = gen_args();
    if let Ok(crontab) = read_crontab(&args.config_path) {
        let crontab = parse_crontab(&crontab);

        /*
        let now = Local::now();
        println!("{}", now);
        */
        let mut handles = Vec::new();
        for entry in crontab {
            handles.push(spawn_entry(&entry));
        }

        for child in handles {

            match child.join() {
                Ok(_) => continue,
                Err(_) => continue,

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
            //println!("{}", String::from_utf8(output.stdout).unwrap());
            //eprintln!("{}", String::from_utf8(output.stderr).unwrap());
            break;
        }
        let next_time = entry.next_attempt();
        let future = Local::now() + Duration::seconds(next_time as i64);
        println!("Scheduling: `{}` for {}", entry.cmd, future);
        let t = Duration::seconds(next_time as i64).to_std().unwrap();
        sleep(t);

        let output = Command::new("/bin/sh")
            .arg("-c")
            .arg(&entry.cmd)
            .output()
            .expect("failed to execute process");
    })
}
