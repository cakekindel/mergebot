use std::fs;

use super::access::AccessRep;

pub trait TokenMgr: 'static + std::fmt::Debug + Sync + Send {
  fn tokens(&self) -> Vec<AccessRep>;
  fn set_tokens(&self, reps: Vec<AccessRep>);

  fn register(&self, rep: &AccessRep) {
    let mut reps = self.tokens();
    reps.push(rep.clone());

    self.set_tokens(reps);
  }

  fn get(&self, team_id: &str) -> Option<String> {
    self.tokens()
        .into_iter()
        .find(|rep| rep.team.id == team_id.as_ref())
        .map(|rep| rep.access_token)
  }
}

#[derive(Debug, Clone, Copy)]
pub struct Fs;
type File = Vec<AccessRep>;

fn read_file() -> File {
  let exist = fs::read_to_string("./access_reps.json").unwrap();
  serde_json::from_str(&exist).unwrap()
}

impl TokenMgr for Fs {
  fn tokens(&self) -> Vec<AccessRep> {
    read_file()
  }

  fn set_tokens(&self, reps: Vec<AccessRep>) {
    let new = serde_json::to_string_pretty(&reps).unwrap();
    fs::write("./access_reps.json", new).unwrap();
  }
}
