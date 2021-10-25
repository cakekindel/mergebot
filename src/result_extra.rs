/// Extra result methods
pub trait ResultExtra<T, E> {
  /// turn Err state into Ok
  fn and_then_err<R, F: Fn(E) -> Result<T, R>>(self, f: F) -> Result<T, R>;
  /// if the result is Ok, pass the value through a predicate.
  /// If predicate fails, map to an error
  fn filter<P: Fn(&T) -> bool, F: Fn(T) -> E>(self, f: P, err: F) -> Result<T, E>;

  /// perform an effect on the Ok variant of the Result
  fn tap<F: FnMut(&T)>(self, f: F) -> Self;

  /// perform an effect on the Err variant of the Result
  fn tap_err<F: FnMut(&E)>(self, f: F) -> Self;
}

impl<T, E> ResultExtra<T, E> for Result<T, E> {
  fn and_then_err<R, F: Fn(E) -> Result<T, R>>(self, f: F) -> Result<T, R> {
    match self {
      | Ok(t) => Ok(t),
      | Err(e) => f(e),
    }
  }

  fn filter<P: Fn(&T) -> bool, F: Fn(T) -> E>(self, f: P, err: F) -> Result<T, E> {
    self.and_then(|t| match f(&t) {
          | true => Ok(t),
          | false => Err(err(t)),
        })
  }

  fn tap<F: FnMut(&T)>(self, mut f: F) -> Self {
    self.map(|ok| {
          f(&ok);
          ok
        })
  }

  fn tap_err<F: FnMut(&E)>(self, mut f: F) -> Self {
    self.map_err(|err| {
          f(&err);
          err
        })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn tap() {
    let mut effect = "none";

    Result::<(), ()>::Ok(())
      .tap(|_| effect = "ok")
      .ok();

    Result::<(), ()>::Err(())
      .tap(|_| panic!("dont call me"))
      .ok();

    assert_eq!(effect, "ok");
  }

  #[test]
  fn tap_err() {
    let mut effect = "none";

    Result::<(), ()>::Err(())
      .tap_err(|_| effect = "err")
      .ok();

    Result::<(), ()>::Ok(())
      .tap_err(|_| panic!("dont call me"))
      .ok();

    assert_eq!(effect, "err");
  }

  #[test]
  fn and_then_err() {
    // is not called on ok
    Result::<&'_ str, &'_ str>::Ok("ok")
      .and_then_err::<(), _>(|_| panic!("should not have been called"))
      .ok();

    // allows transforming err -> ok
    Result::<&'_ str, &'_ str>::Err("oh no")
      .and_then_err::<(), _>(|_| Ok("actually that's fine"))
      .expect("should be ok");

    // allows transforming err -> err
    Result::<&'_ str, &'_ str>::Err("oh no")
      .and_then_err(|_| Err("actually that's not fine"))
      .expect_err("should be err");
 }

  #[test]
  fn filter() {
    // is not called on err
    Result::<&'_ str, &'_ str>::Err("ok")
      .filter(|_| panic!("dont call me"), |_| panic!("dont call me either"))
      .unwrap_err();

    let is_twelve = |n: usize| n == 12;

    // when ok passes pred
    Result::<usize, usize>::Ok(12)
      .filter(|n| is_twelve(*n), |_| panic!("dont call me"))
      .unwrap();

    // when ok fails pred
    Result::<usize, &'_ str>::Ok(14)
      .filter(|n| is_twelve(*n), |_| "not ok")
      .unwrap_err();
 }
}
