/// Extra string stuff
pub trait StrExtra {
  fn loose_eq<O: AsRef<str>>(&self, other: O) -> bool;
}

impl<T: AsRef<str>> StrExtra for T {
  fn loose_eq<O: AsRef<str>>(&self, other: O) -> bool {
    self.as_ref().trim().to_lowercase() == other.as_ref().trim().to_lowercase()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn loose_eq() {
    let a = "    foo ";
    let b = " FoO         \n";

    assert!(a.loose_eq(b));
  }
}
