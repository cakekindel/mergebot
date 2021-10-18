/// Extra string stuff
pub trait StrExtra {
  fn loose_eq<O: AsRef<str>>(&self, other: O) -> bool;
}

impl<T: AsRef<str>> StrExtra for T {
  fn loose_eq<O: AsRef<str>>(&self, other: O) -> bool {
    self.as_ref().trim().to_lowercase() == other.as_ref().trim().to_lowercase()
  }
}
