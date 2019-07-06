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
pub enum CronExprKind {
    Minute,
    Hour,
    DayOfMonth,
    Month,
    DayOfWeek,
}

impl CronExprKind {
    pub fn bounds(&self) -> (u8, u8) {
        match self {
            CronExprKind::Minute => (0, 59),
            CronExprKind::Hour => (0, 23),
            CronExprKind::DayOfMonth => (1, 31),
            CronExprKind::Month => (1, 12),
            CronExprKind::DayOfWeek => (0, 6),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CronInterval {
    Every,
    Multiple(Vec<u8>),
}

impl CronInterval {
    pub fn distance(interval: &CronInterval, current: u8, kind: CronExprKind) -> u8 {
        let (min, max) = kind.bounds();
        match interval {
            CronInterval::Every => 0,
            CronInterval::Multiple(v) => {
                let remainder: Vec<u8> = v
                    .iter()
                    .filter(|x| **x > current)
                    .map(|x| x.clone())
                    .collect();
                if !remainder.is_empty() {
                    return *remainder.iter().nth(0).unwrap();
                }

                // Kill me
                return ((current as i32 - max as i32) as i32 + (*v.iter().nth(0).unwrap()) as i32)
                    as u8;
            }
        }
    }

    pub fn closest(interval: &CronInterval, current: u8, kind: CronExprKind) -> Option<u8> {
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
    // TODO Probably really inefficient method of combining two intervals
    /*
    pub fn combine(&self, other: CronInterval) -> CronInterval {
        use CronInterval::*;
        match self {
            Every => Every,
            Multiple(v1) => match other {
                Every => Every,
                Multiple(v2) => {
                    let mut v = v1.clone();
                    let mut v2 = v2.clone();
                    v.append(&mut v2);
                    Multiple(v)
                }
            },
        }
    }
    */
}

#[derive(Clone, Debug)]
pub struct CronEntry {
    minute: CronInterval,
    hour: CronInterval,
    dom: CronInterval,
    month: CronInterval,
    dow: CronInterval,
    cmd: String,
}

impl CronEntry {
    pub fn next_execution(&self) -> usize {
        let now = Local::now();
        let mut future = now.with_second(0).unwrap();

        let next = CronInterval::closest(&self.minute, now.minute() as u8, CronExprKind::Minute);
        if let Some(t) = next {
            future = future.with_minute((t) as u32).unwrap();
        } else {
            future = future.with_minute(now.minute() + 1).unwrap();
        }

        let next = CronInterval::closest(&self.hour, now.hour() as u8, CronExprKind::Hour);
        if let Some(t) = next {
            future = future.with_hour(t as u32).unwrap();
        }

        /*
        let delta = CronInterval::distance(
            &self.dow,
            now.weekday().num_days_from_sunday() as u8,
            CronExprKind::DayOfWeek,
        );
        future = future + Duration::days(delta as i64);
        */
        // This will sometimes result in faulty values so we do an extra
        // check when we wake up again
        // NEW NOTICE: I abandoned the idea to calculate such distant times
        // months are not easy to work with..
        /*
        let delta = CronInterval::distance(&self.dom, now.day() as u8, CronExprKind::DayOfMonth);
        future = future + Duration::days(delta as i64);

        let delta = CronInterval::distance(&self.month, now.month() as u8, CronExprKind::Month);
        future = future + Duration::months(delta as i64);
        */

        return (future - now).num_seconds() as usize;
    }

    pub fn is_correct_day(&self) -> bool {
        let now = Local::now();

        let correct_month = match &self.month {
            CronInterval::Every => true,
            CronInterval::Multiple(v) => v.contains(&(now.month() as u8)),
        };

        let correct_dow = match &self.dow {
            CronInterval::Every => true,
            CronInterval::Multiple(v) => v.contains(&(now.weekday().num_days_from_sunday() as u8)),
        };

        let correct_dom = match &self.dom {
            CronInterval::Every => true,
            CronInterval::Multiple(v) => v.contains(&(now.day() as u8)),
        };

        return correct_month && correct_dow && correct_dom;

    }
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
            child.join().expect("NO error?");
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
        let next_time = entry.next_execution();
        let now = Local::now() + Duration::seconds(next_time as i64);
        //println!("Scheduling: `{}` for {}", entry.cmd, now);
        sleep(Duration::seconds(next_time as i64).to_std().unwrap());
        if entry.is_correct_day() {
            let output = Command::new("/bin/sh")
                .arg("-c")
                .arg(&entry.cmd)
                .output()
                .expect("failed to execute process");
            //println!("{}", String::from_utf8(output.stdout).unwrap());
            //eprintln!("{}", String::from_utf8(output.stderr).unwrap());
        }

    })
}
