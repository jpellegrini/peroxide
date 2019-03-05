use std::cell::RefCell;

use arena::Arena;
use continuation::Continuation;
use trampoline::Bounce;
use value::Value;

pub fn evaluate(arena: &mut Arena, form: usize, environment: usize, continuation: usize)
                -> Result<Bounce, String> {
  if let Value::Environment(_) = arena.value_ref(environment) {} else {
    panic!("Value passed to evaluate() is not an environment: {:?}", arena.value_ref(environment));
  }

  let val = arena.value_ref(form).clone();
  match val {
    Value::Symbol(s) => evaluate_variable(arena, &s, environment, continuation),
    Value::Pair(_, _) => evaluate_pair(arena, form, environment, continuation),
    Value::EmptyList => Err(format!("Syntax error: applying empty list.")),
    _ => Ok(Bounce::Resume { continuation_r: continuation, value_r: form }),
  }
}

fn evaluate_variable(arena: &mut Arena, name: &str, environment: usize, continuation: usize)
                     -> Result<Bounce, String> {
  if let Value::Environment(e) = arena.value_ref(environment) {
    match e.borrow().get(arena, name) {
      Some(v) => Ok(Bounce::Resume { continuation_r: continuation, value_r: v }),
      None => Err(format!("Undefined value: {}.", name)),
    }
  } else {
    panic!("Value passed to evaluate_variable is not an environment: {:?}", arena.value_ref(environment));
  }
}

fn evaluate_pair(arena: &mut Arena, pair_r: usize, environment: usize, continuation: usize)
                 -> Result<Bounce, String> {
  let pair = arena.value_ref(pair_r).clone();

  if let Value::Pair(car_r, cdr_r) = pair {
    let car = arena.value_ref(*car_r.borrow()).clone();
    if let Value::Symbol(s) = car {
      match s.as_ref() {
        "quote" => evaluate_quote(arena, *cdr_r.borrow(), continuation),
        "if" => evaluate_if(arena, *cdr_r.borrow(), environment, continuation),
        "begin" => evaluate_begin(arena, *cdr_r.borrow(), environment, continuation),
        "lambda" => evaluate_lambda(arena, *cdr_r.borrow(), environment, continuation),
        "set!" => evaluate_set(arena, *cdr_r.borrow(), false, environment, continuation),
        "define" => evaluate_set(arena, *cdr_r.borrow(), true, environment,  continuation),
        _ => evaluate_application(arena, pair_r, environment,  continuation),
      }
    } else {
      evaluate_application(arena, pair_r, environment, continuation)
    }
  } else {
    panic!("Value passed to evaluate_pair() is not a pair: {:?}.", pair);
  }
}

fn evaluate_quote(arena: &mut Arena, cdr_r: usize, continuation: usize)
                  -> Result<Bounce, String> {
  let args = arena.value_ref(cdr_r).pair_to_vec(arena)
      .and_then(|v| with_check_len(v, Some(1), Some(1)))
      .map_err(|s| format!("Syntax error in quote: {}.", s))?;

  Ok(Bounce::Resume { continuation_r: continuation, value_r: args[0] })
}

// TODO (easy: support 2-form version)
fn evaluate_if(arena: &mut Arena, cdr_r: usize, environment: usize, continuation: usize)
               -> Result<Bounce, String> {
  let args = arena.value_ref(cdr_r).pair_to_vec(arena)
      .and_then(|v| with_check_len(v, Some(3), Some(3)))
      .map_err(|s| format!("Syntax error in if: {}.", s))?;

  let cont = Continuation::If {
    e_true: args[1],
    e_false: args[2],
    environment,
    continuation,
  };
  let cont_r = arena.intern(Value::Continuation(RefCell::new(cont)));
  Ok(Bounce::Evaluate { continuation_r: cont_r, value_r: args[0], environment_r: environment })
}


pub fn evaluate_begin(arena: &mut Arena, cdr_r: usize, environment: usize, continuation: usize)
                      -> Result<Bounce, String> {
  let args = arena.value_ref(cdr_r).pair_to_vec(arena)
      .map_err(|s| format!("Syntax error in begin: {}.", s))?;

  match args.len() {
    0 => Ok(Bounce::Resume { continuation_r: continuation, value_r: arena.unspecific }),
    1 => {
      Ok(Bounce::Evaluate { value_r: args[0], environment_r: environment, continuation_r: continuation })
    }
    _ => {
      let cdr = arena.value_ref(cdr_r).cdr();
      let cont = arena.intern_continuation(Continuation::Begin {
        body: cdr,
        environment,
        continuation,
      });
      Ok(Bounce::Evaluate { value_r: args[0], environment_r: environment, continuation_r: cont })
    }
  }
}


fn evaluate_set(arena: &mut Arena, cdr_r: usize, define: bool, environment: usize,
                continuation: usize) -> Result<Bounce, String> {
  let fn_name = if define { "define" } else { "set!" };
  let args = arena.value_ref(cdr_r).pair_to_vec(arena)
      .and_then(|v| with_check_len(v, Some(2), Some(2)))
      .map_err(|s| format!("Syntax error in {}: {}.", fn_name, s))?;

  let name = match arena.value_ref(args[0]) {
    Value::Symbol(s) => s.clone(),
    _ => return Err(format!("Expected symbol, got {}.", arena.value_ref(args[0]).pretty_print(arena)))
  };

  let cont = arena.intern_continuation(
    Continuation::Set { name, define, environment, continuation });

  Ok(Bounce::Evaluate { value_r: args[1], environment_r: environment, continuation_r: cont })
}

// TODO (easy): verify formals at this point
fn evaluate_lambda(arena: &mut Arena, cdr_r: usize, environment: usize, continuation: usize)
                   -> Result<Bounce, String> {
  let args = arena.value_ref(cdr_r).pair_to_vec(arena)
      .and_then(|v| with_check_len(v, Some(2), None))
      .map_err(|s| format!("Syntax error in lambda: {}.", s))?;

  let val = Value::Lambda { environment, formals: args[0], body: arena.value_ref(cdr_r).cdr() };
  Ok(Bounce::Resume { continuation_r: continuation, value_r: arena.intern(val) })
}


fn evaluate_application(arena: &mut Arena, cdr_r: usize, environment: usize, continuation: usize)
                        -> Result<Bounce, String> {
  let args = arena.value_ref(cdr_r).pair_to_vec(arena)
      .map_err(|s| format!("Syntax error in application: {}.", s))?;

  if args.is_empty() {
    panic!("Syntax error in application: empty list. This should have been caught before.")
  }

  let fun_args = arena.value_ref(cdr_r).cdr();
  let cont = arena.intern_continuation(
    Continuation::EvFun { args: fun_args, environment, continuation });
  Ok(Bounce::Evaluate { environment_r: environment, value_r: args[0], continuation_r: cont })
}


pub fn evaluate_arguments(arena: &mut Arena, environment: usize, args_r: usize, continuation: usize)
                          -> Result<Bounce, String> {
  let args = arena.value_ref(args_r).pair_to_vec(arena)
      .expect(&format!("Argument evaluation didn't produce a list."));

  if args.is_empty() {
    Ok(Bounce::Resume { continuation_r: continuation, value_r: arena.empty_list })
  } else {
    let cont = arena.intern_continuation(Continuation::Argument {
      sequence: args_r,
      environment,
      continuation,
    });
    Ok(Bounce::Evaluate { environment_r: environment, value_r: args[0], continuation_r: cont })
  }
}

fn with_check_len<T>(v: Vec<T>, min: Option<usize>, max: Option<usize>) -> Result<Vec<T>, String> {
  match min {
    Some(m) => if v.len() < m { return Err(format!("Too few values, expecting at least {}", m)); },
    _ => ()
  };
  match max {
    Some(m) => if v.len() > m { return Err(format!("Too many values, expecting at most {}", m)); },
    _ => ()
  }
  Ok(v)
}
