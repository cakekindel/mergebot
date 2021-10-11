pub trait ResultExtra<T, E> {
  fn and_then_err<R>(self, f: impl Fn(E) -> Result<T, R>) -> Result<T, R>;
  fn filter(self, f: impl Fn(&T) -> bool, err: impl Fn(T) -> E) -> Result<T, E>;
}

impl<T, E> ResultExtra<T, E> for Result<T, E> {
  fn and_then_err<R>(self, f: impl Fn(E) -> Result<T, R>) -> Result<T, R> {
    match self {
      | Ok(t) => Ok(t),
      | Err(e) => f(e),
    }
  }

  fn filter(self, f: impl Fn(&T) -> bool, err: impl Fn(T) -> E) -> Result<T, E> {
    self.and_then(|t| match f(&t) {
          | true => Ok(t),
          | false => Err(err(t)),
        })
  }
}
