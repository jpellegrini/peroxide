extern crate rustyline;

use std::cell::RefCell;
use std::env;

use arena::Arena;
use ast::to_syntax_element;
use continuation::Continuation;
use environment::Environment;
use lex::SegmentationResult;
use lex::Token;
use primitives::register_primitives;
use repl::GetLineError;
use repl::{FileRepl, ReadlineRepl, Repl, StdIoRepl};
use trampoline::evaluate_toplevel;
use value::Value;

mod arena;
mod ast;
mod continuation;
mod environment;
mod eval;
mod lex;
mod parse;
mod primitives;
mod repl;
mod trampoline;
mod util;
mod value;

fn main() {
    let args: Vec<String> = env::args().collect();
    match do_main(args) {
        Err(e) => {
            println!("Error: {}", e);
            std::process::exit(1)
        }
        Ok(()) => std::process::exit(0),
    }
}

fn do_main(args: Vec<String>) -> Result<(), String> {
    let options = parse_args(&args.iter().map(|x| &**x).collect::<Vec<_>>())
        .map_err(|e| format!("Could not parse arguments: {}", e))?;

    let mut repl: Box<Repl> = match options.input_file {
        Some(f) => Box::new(FileRepl::new(&f)?),
        None => {
            if options.enable_readline {
                Box::new(ReadlineRepl::new(Some("history.txt".to_string())))
            } else {
                Box::new(StdIoRepl {})
            }
        }
    };

    let mut arena = Arena::new();

    let mut environment = Environment::new(None);
    register_primitives(&mut arena, &mut environment);
    let environment_r = arena.intern(Value::Environment(RefCell::new(environment)));

    let cont = arena.intern_continuation(Continuation::TopLevel);

    loop {
        let result = handle_one_expr_wrap(&mut *repl, &mut arena, environment_r, cont);
        if !result {
            break;
        }
    }

    repl.save_history();
    Ok(())
}

// Returns true if the REPL loop should continue, false otherwise.
fn handle_one_expr_wrap(
    repl: &mut Repl,
    arena: &mut Arena,
    environment: usize,
    continuation: usize,
) -> bool {
    handle_one_expr(repl, arena, environment, continuation)
        .map_err(|e| println!("Error: {}", e))
        .unwrap_or(true)
}

fn handle_one_expr(
    repl: &mut Repl,
    arena: &mut Arena,
    environment: usize,
    continuation: usize,
) -> Result<bool, String> {
    let mut current_expr_string: Vec<String> = Vec::new();
    let mut exprs: Vec<Vec<Token>> = Vec::new();
    let mut pending_expr: Vec<Token> = Vec::new();
    let mut depth: u64 = 0;

    loop {
        let line_opt = if pending_expr.is_empty() {
            repl.get_line(">>> ", "")
        } else {
            repl.get_line("... ", &" ".to_string().repeat((depth * 2) as usize))
        };

        match line_opt {
            Err(GetLineError::Eof) => return Ok(false),
            Err(GetLineError::Interrupted) => return Ok(false),
            Err(GetLineError::Err(s)) => {
                println!("Readline error: {}", s);
                return Ok(true);
            }
            Ok(_) => (),
        };

        let line = line_opt.unwrap();
        let mut tokenize_result = lex::lex(&line)?;
        current_expr_string.push(line);
        pending_expr.append(&mut tokenize_result);

        let SegmentationResult {
            mut segments,
            remainder,
            depth: new_depth,
        } = lex::segment(pending_expr)?;
        exprs.append(&mut segments);

        if remainder.is_empty() {
            break;
        }

        depth = new_depth;
        pending_expr = remainder;
    }

    repl.add_to_history(&current_expr_string.join("\n"));
    rep(arena, exprs, environment, continuation);
    Ok(true)
}

fn rep(arena: &mut Arena, toks: Vec<Vec<Token>>, environment_r: usize, cont_r: usize) {
    for token_vector in toks {
        match parse::parse(arena, &token_vector) {
            Ok(value) => {
                let value_r = arena.intern(value);
                let result = ast::to_syntax_element(arena, value_r).map(|v| format!("{:?}", v));
                match result {
                    Ok(x) => println!(" => {}", x),
                    Err(x) => println!(" !> {}", x),
                }
            }
            Err(s) => println!("Parsing error: {:?}", s),
        }
    }
}

#[derive(Debug)]
struct Options {
    pub enable_readline: bool,
    pub input_file: Option<String>,
}

// TODO emit sensible error / warning messages
fn parse_args(args: &[&str]) -> Result<Options, String> {
    let (mut positional, flags): (Vec<&str>, Vec<&str>) =
        args.iter().skip(1).partition(|s| !s.starts_with("--"));

    let enable_readline = !flags.iter().any(|&x| x == "--no-readline");
    let input_file = if positional.len() <= 1 {
        positional.pop().map(|x| x.to_string())
    } else {
        return Err("Too many positional arguments.".into());
    };
    Ok(Options {
        enable_readline,
        input_file,
    })
}
