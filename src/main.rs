extern crate core;
extern crate rustyline;

use std::env;

use arena::Arena;
use environment::{Environment, RcEnv};
use lex::SegmentationResult;
use lex::Token;
use repl::GetLineError;
use repl::{FileRepl, ReadlineRepl, Repl, StdIoRepl};
use std::cell::RefCell;
use std::rc::Rc;
use vm::Instruction;

mod arena;
mod ast;
mod compile;
mod environment;
mod lex;
mod parse;
mod primitives;
mod repl;
mod util;
mod value;
mod vm;

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
    primitives::register_primitives(&mut environment);
    let rc_env = Rc::new(RefCell::new(environment));

    loop {
        if !handle_one_expr_wrap(&mut *repl, &mut arena, rc_env.clone()) {
            break;
        }
    }

    repl.save_history();
    Ok(())
}

// Returns true if the REPL loop should continue, false otherwise.
fn handle_one_expr_wrap(repl: &mut Repl, arena: &mut Arena, environment: RcEnv) -> bool {
    handle_one_expr(repl, arena, environment)
        .map_err(|e| println!("Error: {}", e))
        .unwrap_or(true)
}

fn handle_one_expr(repl: &mut Repl, arena: &mut Arena, environment: RcEnv) -> Result<bool, String> {
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
    rep(arena, exprs, environment);
    Ok(true)
}

fn rep(arena: &mut Arena, toks: Vec<Vec<Token>>, environment: RcEnv) -> Result<(), ()> {
    for token_vector in toks {
        let parse_value =
            parse::parse(arena, &token_vector).map_err(|e| println!("Parsing error: {:?}", e))?;
        let value_r = arena.intern(parse_value);
        let syntax_tree =
            ast::to_syntax_element(arena, value_r).map_err(|e| println!("Syntax error: {}", e))?;
        println!(" => {:?}", syntax_tree);
        let mut compiled = Vec::new();
        compile::compile(&syntax_tree, &mut compiled, environment.clone(), true)
            .map_err(|e| println!("Compilation error: {}", e))?;
        compiled.push(Instruction::Finish);
        println!(" => {:?}", compiled);
        match vm::run(arena, &compiled, 0) {
            Ok(v) => println!(" => {}", arena.value_ref(v).pretty_print(arena)),
            Err(e) => println!("Runtime error: {:?}", e),
        }
    }
    Ok(())
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
