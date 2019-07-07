#[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
pub struct CronExpr(pub CronUnit, pub CronInterval);

impl CronUnit {
  pub fn max(&self) -> u32 {
    use CronUnit::*;
    match self {
      Minute => 59,
      Hour => 23,
      DayOfMonth => 31,
      Month => 12,
      DayOfWeek => 6,
    }
  }
  pub fn min(&self) -> u32 {
    use CronUnit::*;
    match self {
      Minute => 0,
      Hour => 0,
      DayOfMonth => 1,
      Month => 1,
      DayOfWeek => 0,
    }
  }
  pub fn bounds(&self) -> (u32, u32) {
    (self.min(), self.max())
  }
}

impl CronExpr {
  pub fn contains(&self, now: u32) -> bool {
    let CronExpr(_, interval) = self;
    match interval {
      CronInterval::Every => true,
      CronInterval::Multiple(v) => v.contains(&now),
    }
  }
  pub fn next_from(&self, now: u32) -> u32 {
    let CronExpr(unit, interval) = self;
    match interval {
      CronInterval::Every => {
        let n = (now + 1) % (unit.max() + 1);
        if n == 0 {
          return n + unit.min();
        }
        return n;
      }
      CronInterval::Multiple(v) => {
        if let Some(target) = v.iter().filter(|x| x > &&now).nth(0) {
          return *target;
        }
        return *v.iter().nth(0).unwrap();
      }
    }
  }

  pub fn is_every(&self) -> bool {
    match self {
      CronExpr(_, CronInterval::Every) => true,
      _ => false,
    }
  }

  pub fn is_multiple(&self) -> bool {
    !self.is_every()
  }
}


#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn contains() {
    let v = vec![5, 15, 20];
    let ce = CronExpr(CronUnit::Minute, CronInterval::Multiple(v));
    assert!(ce.contains(5));
    assert!(ce.contains(15));
    assert!(ce.contains(20));
    assert!(!ce.contains(4));
    assert!(!ce.contains(98));
    assert!(!ce.contains(21));
  }

  #[test]
  fn next_from() {
    let v = vec![5, 15, 20];
    let ce = CronExpr(CronUnit::Minute, CronInterval::Multiple(v));
    assert_eq!(ce.next_from(5), 15);
    assert_eq!(ce.next_from(14), 15);
    assert_eq!(ce.next_from(25), 5);
    assert_eq!(ce.next_from(65), 5);

    let ce = CronExpr(CronUnit::Minute, CronInterval::Every);
    assert_eq!(ce.next_from(5), 6);
    assert_eq!(ce.next_from(0), 1);
    assert_eq!(ce.next_from(59), 0);

    let ce = CronExpr(CronUnit::Month, CronInterval::Every);
    assert_eq!(ce.next_from(5), 6);
    assert_eq!(ce.next_from(1), 2);
    assert_eq!(ce.next_from(12), 1);
  }
}