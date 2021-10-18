/// Extra string stuff
pub trait StrExtra {
  fn loose_eq<O: AsRef<str>>(&self, other: O) -> bool;
}

impl<T: AsRef<str>> StrExtra for T {
  fn loose_eq<O: AsRef<str>>(&self, other: O) -> bool {
    a.as_ref().trim().to_lowercase() == b.as_ref().trim().to_lowercase()
  }
}
