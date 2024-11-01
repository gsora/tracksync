use crate::model;
use rhai::{Engine, Scope, AST};

#[derive(Debug)]
pub enum Error {
    ParseError(String),
    RunError(String),
    RegexError(regex::Error),
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::ParseError(e) => write!(f, "{}", e),
            Error::RunError(e) => write!(f, "{}", e),
            Error::RegexError(e) => write!(f, "{}", e),
        }
    }
}

impl From<rhai::ParseError> for Error {
    fn from(value: rhai::ParseError) -> Self {
        Error::ParseError(format!("{}", value))
    }
}

impl From<rhai::EvalAltResult> for Error {
    fn from(value: rhai::EvalAltResult) -> Self {
        Error::RunError(format!("{}", value))
    }
}

impl From<regex::Error> for Error {
    fn from(value: regex::Error) -> Self {
        Error::RegexError(value)
    }
}

// fn filter(track: model::BaseTrack)
const FILTER_FN_NAME: &'static str = "filter";

pub struct ScriptRuntime {
    ast: AST,
    engine: Engine,
}

impl ScriptRuntime {
    pub fn run(&self, model: Vec<model::BaseTrack>) -> Result<Vec<bool>, Error> {
        let mut ret = vec![];
        for track in model {
            let mut scope = Scope::new();
            let res = self
                .engine
                .call_fn::<bool>(&mut scope, &self.ast, FILTER_FN_NAME, (track,));

            match res {
                Ok(res) => ret.push(res),
                Err(result) => return Err((*result).into()),
            };
        }

        Ok(ret)
    }
}

pub fn check(scripts: Vec<String>) -> Result<(), Error> {
    match evaluate(scripts) {
        Err(e) => Err(e),
        Ok(_) => Ok(()),
    }
}

pub fn evaluate(scripts: Vec<String>) -> Result<Vec<ScriptRuntime>, Error> {
    let mut sc = vec![];

    for s in scripts {
        sc.push(compile(s)?)
    }

    Ok(sc)
}

fn compile(script: String) -> Result<ScriptRuntime, Error> {
    let mut engine = Engine::new();

    engine.register_fn("regex_match", regex_match);
    engine.build_type::<model::BaseTrack>();
    engine
        .register_type_with_name::<Vec<model::BaseTrack>>("VecTrack")
        .register_iterator::<Vec<model::BaseTrack>>();
    let ast = engine.compile(&script)?;

    Ok(ScriptRuntime { ast, engine })
}

fn regex_match(expr: String, data: String) -> bool {
    let r = regex::Regex::new(&expr).unwrap();

    r.is_match(&data)
}
