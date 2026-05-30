use std::{
    collections::{HashMap, HashSet},
    fs::*,
    io::prelude::*,
    io::*,
    process::exit,
    sync::LazyLock,
};
use wasmtime::{Engine, Module};

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The input source file (.rem) to compile
    #[arg(required = true)]
    input_file: PathBuf,

    /// Dumps the AST in the stdout
    #[arg(long, default_value_t = false)]
    ast: bool,

    /// Specify the name of the output executable binary
    #[arg(short, long, default_value = "")]
    output: String,

    /// Autorun the final executable program
    #[arg(short, long, default_value_t = false)]
    run: bool,
}

#[derive(Debug, Clone)]
struct Symbol {
    pub name: String,
    pub data_type: ASTType,
}

#[derive(Default, Debug, Clone)]
struct SymbolTable {
    pub entries: HashMap<String, Symbol>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn insert(&mut self, name: String, data_type: ASTType) -> bool {
        if self.entries.contains_key(&name) {
            return false;
        }
        self.entries
            .insert(name.clone(), Symbol { name, data_type });
        true
    }

    pub fn lookup(&self, name: &str) -> Option<&Symbol> {
        self.entries.get(name)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[derive(Debug, Clone, PartialEq)]
enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone)]
enum ASTExpr {
    Int(i32),
    Variable(String),
    Binary(Box<ASTExpr>, BinaryOp, Box<ASTExpr>),
    Call(String, Vec<ASTExpr>),
}

impl Default for ASTExpr {
    fn default() -> Self {
        ASTExpr::Int(0)
    }
}

#[derive(Default, Debug, Clone)]
struct ASTLetStmt {
    var_name: String,
    value: ASTExpr,
}

#[derive(Default, Debug, Clone)]
struct ASTReturnStmt {
    value: ASTExpr,
    function_ret_type: ASTType,
}

#[derive(Default, Debug, Clone)]
struct ASTAssignmentStmt {
    var_name: String,
    value: ASTExpr,
}

#[derive(Debug, Clone)]
enum ASTStmt {
    LetStmt(ASTLetStmt),
    ReturnStmt(ASTReturnStmt),
    AssignmentStmt(ASTAssignmentStmt),
}

#[derive(Default, Debug, Clone, PartialEq)]
enum ASTType {
    #[default]
    Void,
    I32,
}

#[derive(Debug, Clone, Default)]
struct ASTParam {
    name: String,
    data_type: ASTType,
}

#[derive(Default, Debug, Clone)]
struct ASTFuncDef {
    name: String,
    params: Vec<ASTParam>,
    return_type: ASTType,
    body: Vec<ASTStmt>,
}

#[derive(Debug, Clone)]
enum ASTNode {
    FuncDef(ASTFuncDef),
}

#[derive(Debug, PartialEq, Clone)]
enum Token {
    Ident(String),
    Int(i32),
    Return,
    Fn,
    Let,
    Equals,
    Semicolon,
    OpenParen,
    CloseParen,
    OpenCurly,
    CloseCurly,
    Plus,
    Minus,
    Asterisk,
    Slash,
    Colon,
    Comma,
    EOF,
}

static KEY_CHARS: LazyLock<Vec<u8>> = LazyLock::new(|| {
    vec![
        b'(', b')', b'{', b'}', b'=', b';', b'+', b'-', b'*', b'/', b':', b',',
    ]
});

struct Compiler {
    cur_tok: Token,
    src: Vec<u8>,
    index: usize,
    line: usize,
    program: Vec<ASTNode>,
    vars: SymbolTable,
    functions: HashMap<String, ASTFuncDef>,
}

impl Compiler {
    fn new(src: String) -> Compiler {
        Compiler {
            cur_tok: Token::EOF,
            src: src.into_bytes(),
            index: 0,
            line: 1,
            program: Vec::new(),
            vars: SymbolTable::new(),
            functions: HashMap::new(),
        }
    }

    fn lexe(&mut self) {
        while self.index < self.src.len() && self.src[self.index].is_ascii_whitespace() {
            if self.src[self.index] == b'\n' {
                self.line += 1;
            }
            self.index += 1;
        }

        if self.index >= self.src.len() {
            self.cur_tok = Token::EOF;
            return;
        }

        let c = self.src[self.index];

        match c {
            b'(' => {
                self.cur_tok = Token::OpenParen;
                self.index += 1;
            }
            b')' => {
                self.cur_tok = Token::CloseParen;
                self.index += 1;
            }
            b'{' => {
                self.cur_tok = Token::OpenCurly;
                self.index += 1;
            }
            b'}' => {
                self.cur_tok = Token::CloseCurly;
                self.index += 1;
            }
            b'=' => {
                self.cur_tok = Token::Equals;
                self.index += 1;
            }
            b';' => {
                self.cur_tok = Token::Semicolon;
                self.index += 1;
            }
            b'+' => {
                self.cur_tok = Token::Plus;
                self.index += 1;
            }
            b'-' => {
                self.cur_tok = Token::Minus;
                self.index += 1;
            }
            b'*' => {
                self.cur_tok = Token::Asterisk;
                self.index += 1;
            }
            b'/' => {
                self.cur_tok = Token::Slash;
                self.index += 1;
            }
            b':' => {
                self.cur_tok = Token::Colon;
                self.index += 1;
            }
            b',' => {
                self.cur_tok = Token::Comma;
                self.index += 1;
            }
            _ => {
                if c.is_ascii_alphabetic() {
                    let mut buff: Vec<u8> = Vec::new();

                    while self.index < self.src.len() {
                        let current_byte = self.src[self.index];

                        if (current_byte.is_ascii_alphanumeric() || current_byte == b'_')
                            && !KEY_CHARS.contains(&current_byte)
                        {
                            buff.push(current_byte);
                            self.index += 1;
                        } else {
                            break;
                        }
                    }

                    match String::from_utf8(buff) {
                        Ok(str) => match str.as_str() {
                            "fn" => self.cur_tok = Token::Fn,
                            "let" => self.cur_tok = Token::Let,
                            "return" => self.cur_tok = Token::Return,
                            _ => self.cur_tok = Token::Ident(str),
                        },
                        Err(e) => {
                            println!("ERROR [line: {}]: Invalid UTF-8 bytes: {}", self.line, e);
                            exit(1);
                        }
                    }
                } else if c.is_ascii_digit() {
                    let mut buff: Vec<u8> = Vec::new();

                    while self.index < self.src.len() {
                        let current_byte = self.src[self.index];

                        if current_byte.is_ascii_digit() && !KEY_CHARS.contains(&current_byte) {
                            buff.push(current_byte);
                            self.index += 1;
                        } else {
                            break;
                        }
                    }

                    match String::from_utf8(buff) {
                        Ok(str) => self.cur_tok = Token::Int(str.parse().unwrap()),
                        Err(e) => {
                            println!("ERROR [line: {}]: Invalid UTF-8 bytes: {}", self.line, e);
                            exit(1);
                        }
                    }
                } else {
                    println!(
                        "ERROR [line: {}]: Unknow character: '{}'",
                        self.line, c as char
                    );
                    exit(1);
                }
            }
        }
    }

    fn build_ast(&mut self) {
        self.lexe();

        while self.cur_tok != Token::EOF {
            match self.cur_tok {
                Token::Fn => self.parse_fn_def(),
                _ => {
                    println!(
                        "ERROR [line: {}]: Unexcpeted operation in global scope: {:?}",
                        self.line, self.cur_tok
                    );
                    exit(1);
                }
            }
            self.lexe();
        }
    }

    fn parse_fn_def(&mut self) {
        self.lexe();

        let mut function = ASTFuncDef::default();

        if let Token::Ident(ref name) = self.cur_tok {
            function.name = name.to_string();
        } else {
            println!(
                "ERROR [line: {}]: Excpected a function name got: {:?}",
                self.line, self.cur_tok
            );
            exit(1);
        }

        self.lexe();
        if self.cur_tok != Token::OpenParen {
            println!(
                "ERROR [line: {}]: Excpected '(' after function name got: {:?}",
                self.line, self.cur_tok
            );
            exit(1);
        }

        self.lexe();
        self.vars.clear();

        if self.cur_tok != Token::CloseParen {
            loop {
                let mut param = ASTParam::default();

                if let Token::Ident(ref p_name) = self.cur_tok {
                    param.name = p_name.to_string();
                } else {
                    println!(
                        "ERROR [line: {}]: Expected parameter name, got {:?}",
                        self.line, self.cur_tok
                    );
                    exit(1);
                }
                self.lexe();

                if self.cur_tok != Token::Colon {
                    println!(
                        "ERROR [line: {}]: Expected ':' after parameter name, got {:?}",
                        self.line, self.cur_tok
                    );
                    exit(1);
                }
                self.lexe();

                if let Token::Ident(ref p_type) = self.cur_tok {
                    match p_type.as_str() {
                        "i32" => param.data_type = ASTType::I32,
                        _ => {
                            println!(
                                "ERROR [line: {}]: Unknown parameter type: {}",
                                self.line, p_type
                            );
                            exit(1);
                        }
                    }
                } else {
                    println!(
                        "ERROR [line: {}]: Expected parameter type, got {:?}",
                        self.line, self.cur_tok
                    );
                    exit(1);
                }
                self.lexe();

                function.params.push(param.clone());
                self.vars.insert(param.name, param.data_type);

                if self.cur_tok == Token::Comma {
                    self.lexe();
                } else if self.cur_tok == Token::CloseParen {
                    break;
                } else {
                    println!(
                        "ERROR [line: {}]: Expected ',' or ')' inside parameter list, got {:?}",
                        self.line, self.cur_tok
                    );
                    exit(1);
                }
            }
        }

        if self.cur_tok != Token::CloseParen {
            println!(
                "ERROR [line: {}]: Expected ')' after parameter definitions, got: {:?}",
                self.line, self.cur_tok
            );
            exit(1);
        }
        self.lexe();

        self.functions
            .insert(function.name.clone(), function.clone());

        if let Token::Ident(ref ret_type) = self.cur_tok {
            match ret_type.as_str() {
                "i32" => function.return_type = ASTType::I32,
                "void" => function.return_type = ASTType::Void,
                _ => {
                    println!(
                        "ERROR [line: {}]: Unknow return type: {}",
                        self.line, ret_type
                    );
                    exit(1);
                }
            }
            self.lexe();
        }

        if self.cur_tok != Token::OpenCurly {
            println!(
                "ERROR [line: {}]: Excpected '{{' after function name or function return type got: {:?}",
                self.line, self.cur_tok
            );
            exit(1);
        }

        self.lexe();
        while self.cur_tok != Token::CloseCurly {
            function.body.push(self.parse_stmt(&function.return_type));
        }

        self.program.push(ASTNode::FuncDef(function));
    }

    fn parse_stmt(&mut self, return_type: &ASTType) -> ASTStmt {
        let stmt;

        match &self.cur_tok {
            Token::Let => {
                stmt = self.parse_let_stmt();
            }
            Token::Return => {
                stmt = self.parse_return_stmt(return_type);
            }
            Token::Ident(name) => {
                if let Some(symbol) = self.vars.lookup(name) {
                    if symbol.data_type != ASTType::I32 {
                        println!("ERROR [line: {}]: Type mismatch!", self.line);
                        exit(1);
                    }
                } else {
                    println!(
                        "ERROR [line: {}]: Variable '{}' not declared before assignment.",
                        self.line, name
                    );
                    exit(1);
                }
                stmt = self.parse_assignment_stmt();
            }
            _ => {
                println!(
                    "ERROR [line: {}]: Unknow statement: {:?}",
                    self.line, self.cur_tok
                );
                exit(1);
            }
        }

        if self.cur_tok != Token::Semicolon {
            println!(
                "ERROR [line: {}]: Excpected ';' after statement got: {:?}",
                self.line, self.cur_tok
            );
            exit(1);
        }

        self.lexe();
        stmt
    }

    fn parse_assignment_stmt(&mut self) -> ASTStmt {
        let mut stmt = ASTAssignmentStmt::default();

        if let Token::Ident(name) = &self.cur_tok {
            stmt.var_name = name.clone();
        }
        self.lexe();

        if self.cur_tok != Token::Equals {
            println!(
                "ERROR [line: {}]: Expected '=' after variable name, got {:?}",
                self.line, self.cur_tok
            );
            exit(1);
        }
        self.lexe();

        stmt.value = self.parse_expr(0);

        ASTStmt::AssignmentStmt(stmt)
    }

    fn parse_return_stmt(&mut self, return_type: &ASTType) -> ASTStmt {
        let mut r#return = ASTReturnStmt::default();
        self.lexe();

        match return_type {
            &ASTType::Void => {
                if self.cur_tok != Token::Semicolon {
                    println!(
                        "ERROR [line: {}]: Excpected empty return statement for void function got: {:?}",
                        self.line, self.cur_tok
                    );
                    exit(1);
                }
            }
            &ASTType::I32 => {
                r#return.value = self.parse_expr(0);
            }
        }

        r#return.function_ret_type = return_type.clone();
        return ASTStmt::ReturnStmt(r#return);
    }

    fn parse_let_stmt(&mut self) -> ASTStmt {
        let mut variable = ASTLetStmt::default();
        self.lexe();

        if let Token::Ident(ref name) = self.cur_tok {
            variable.var_name = name.to_string();
        } else {
            println!(
                "ERROR [line: {}]: Excpected a variable name got: {:?}",
                self.line, self.cur_tok
            );
            exit(1);
        }

        self.lexe();
        if self.cur_tok != Token::Equals {
            println!(
                "ERROR [line: {}]: Excpected '=' after variable name got: {:?}",
                self.line, self.cur_tok
            );
            exit(1);
        }

        self.lexe();
        variable.value = self.parse_expr(0);

        self.vars.insert(variable.var_name.clone(), ASTType::I32);

        ASTStmt::LetStmt(variable)
    }

    fn token_precedence(&self, tok: &Token) -> u32 {
        match tok {
            Token::Plus | Token::Minus => 1,
            Token::Asterisk | Token::Slash => 2,
            _ => 0,
        }
    }

    fn parse_expr(&mut self, min_precedence: u32) -> ASTExpr {
        let mut left = match self.cur_tok.clone() {
            Token::Int(val) => {
                self.lexe();
                ASTExpr::Int(val)
            }
            Token::Ident(name) => {
                self.lexe();

                if self.cur_tok == Token::OpenParen {
                    self.lexe();

                    let mut args = Vec::new();

                    if self.cur_tok != Token::CloseParen {
                        loop {
                            let arg_expr = self.parse_expr(0);
                            args.push(arg_expr);

                            if self.cur_tok == Token::Comma {
                                self.lexe();
                            } else if self.cur_tok == Token::CloseParen {
                                break;
                            } else {
                                println!(
                                    "ERROR [line: {}]: Expected ',' or ')' in function call arguments, got {:?}",
                                    self.line, self.cur_tok
                                );
                                exit(1);
                            }
                        }
                    }

                    if self.cur_tok != Token::CloseParen {
                        println!(
                            "ERROR [line: {}]: Expected ')' after function call arguments",
                            self.line
                        );
                        exit(1);
                    }
                    self.lexe();

                    if let Some(target_fn) = self.functions.get(&name) {
                        if target_fn.params.len() != args.len() {
                            println!(
                                "ERROR [line: {}]: Function '{}' expects {} arguments, but {} were provided.",
                                self.line,
                                name,
                                target_fn.params.len(),
                                args.len()
                            );
                            exit(1);
                        }
                    } else {
                        println!(
                            "ERROR [line: {}]: Call to undeclared function '{}'",
                            self.line, name
                        );
                        exit(1);
                    }

                    ASTExpr::Call(name, args)
                } else {
                    if self.vars.lookup(&name).is_none() {
                        println!(
                            "ERROR [line: {}]: Use of undeclared identifier '{}'",
                            self.line, name
                        );
                        exit(1);
                    }
                    ASTExpr::Variable(name)
                }
            }
            Token::OpenParen => {
                self.lexe();
                let expr = self.parse_expr(0);
                if self.cur_tok != Token::CloseParen {
                    println!(
                        "ERROR [line: {}]: Expected matching ')' close brace",
                        self.line
                    );
                    exit(1);
                }
                self.lexe();
                expr
            }
            _ => {
                println!(
                    "ERROR [line: {}]: Expected value expression expression component, got: {:?}",
                    self.line, self.cur_tok
                );
                exit(1);
            }
        };

        loop {
            let prec = self.token_precedence(&self.cur_tok);
            if prec == 0 || prec <= min_precedence {
                break;
            }

            let op = match self.cur_tok {
                Token::Plus => BinaryOp::Add,
                Token::Minus => BinaryOp::Sub,
                Token::Asterisk => BinaryOp::Mul,
                Token::Slash => BinaryOp::Div,
                _ => break,
            };

            self.lexe();

            let right = self.parse_expr(prec);
            left = ASTExpr::Binary(Box::new(left), op, Box::new(right));
        }

        left
    }
}

struct Backend {
    program: Vec<ASTNode>,
    wat_file: File,
}

impl Backend {
    fn new(wat_file_name: String, program: Vec<ASTNode>) -> Backend {
        let wat_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(wat_file_name)
            .unwrap();

        Backend { program, wat_file }
    }

    fn compile_program(&mut self) {
        self.wat_file.write_all(b"(module\n").unwrap();
        let program = self.program.clone();

        for ast in &program {
            match ast {
                ASTNode::FuncDef(func) => self.compile_func_def(func),
            }
        }
        self.wat_file.write_all(b")\n").unwrap();

        self.wat_file.flush().unwrap();
        self.wat_file.seek(SeekFrom::Start(0)).unwrap();
    }

    fn compile_func_def(&mut self, func: &ASTFuncDef) {
        let mut params_wat = String::new();
        for param in &func.params {
            params_wat.push_str(&format!(" (param ${} i32)", param.name));
        }

        let output = format!(
            "\t(func ${} (export \"{}\"){}{}\n",
            func.name,
            func.name,
            params_wat,
            self.type_to_string(&func.return_type)
        );
        self.wat_file.write_all(output.as_bytes()).unwrap();

        let mut declared_locals = HashSet::new();
        for stmt in &func.body {
            if let ASTStmt::LetStmt(variable) = stmt {
                if declared_locals.insert(variable.var_name.clone()) {
                    writeln!(self.wat_file, "\t\t(local ${} i32)", variable.var_name).unwrap();
                }
            }
        }

        for stmt in &func.body {
            self.compile_stmt(stmt);
        }

        self.wat_file.write_all(b"\t)\n").unwrap();
    }

    fn compile_stmt(&mut self, stmt: &ASTStmt) {
        match stmt {
            ASTStmt::LetStmt(variable) => {
                self.compile_expr(&variable.value);

                let output = format!("\t\tlocal.set ${}\n", variable.var_name);
                self.wat_file.write_all(output.as_bytes()).unwrap();
            }
            ASTStmt::ReturnStmt(r#return) => match r#return.function_ret_type {
                ASTType::Void => {
                    self.wat_file.write_all(b"\t\treturn\n").unwrap();
                }
                ASTType::I32 => {
                    self.compile_expr(&r#return.value);
                    self.wat_file.write_all(b"\t\treturn\n").unwrap();
                }
            },
            ASTStmt::AssignmentStmt(variable) => {
                self.compile_expr(&variable.value);
                let output = format!("\t\tlocal.set ${}\n", variable.var_name);
                self.wat_file.write_all(output.as_bytes()).unwrap();
            }
        }
    }

    fn compile_expr(&mut self, expr: &ASTExpr) {
        match expr {
            ASTExpr::Int(val) => {
                let output = format!("\t\ti32.const {}\n", val);
                self.wat_file.write_all(output.as_bytes()).unwrap();
            }
            ASTExpr::Variable(name) => {
                let output = format!("\t\tlocal.get ${}\n", name);
                self.wat_file.write_all(output.as_bytes()).unwrap();
            }
            ASTExpr::Binary(left, op, right) => {
                self.compile_expr(left);
                self.compile_expr(right);

                let instr = match op {
                    BinaryOp::Add => "\t\ti32.add\n",
                    BinaryOp::Sub => "\t\ti32.sub\n",
                    BinaryOp::Mul => "\t\ti32.mul\n",
                    BinaryOp::Div => "\t\ti32.div_s\n",
                };
                self.wat_file.write_all(instr.as_bytes()).unwrap();
            }
            ASTExpr::Call(func_name, args) => {
                for arg in args {
                    self.compile_expr(arg);
                }

                let output = format!("\t\tcall ${}\n", func_name);
                self.wat_file.write_all(output.as_bytes()).unwrap();
            }
        }
    }

    fn type_to_string(&mut self, r#type: &ASTType) -> String {
        match r#type {
            &ASTType::I32 => String::from("(result i32)"),
            &ASTType::Void => String::from(""),
        }
    }
}

fn main() {
    let args = Args::parse();

    let output_file = if args.output.is_empty() {
        let mut path = PathBuf::from("out");
        if std::env::consts::OS == "windows" {
            path.set_extension("exe");
        }
        path.to_string_lossy().into_owned()
    } else {
        if std::env::consts::OS == "windows" && !args.output.ends_with(".exe") {
            format!("{}.exe", args.output)
        } else {
            args.output
        }
    };

    match args.input_file.extension() {
        Some(ext) if ext == "rem" => {}
        _ => {
            eprintln!("ERROR: Invalid file extension. The Rem compiler only accepts .rem files.");
            exit(1);
        }
    }

    if !args.input_file.exists() {
        println!(
            "ERROR: could not find file {}",
            args.input_file.to_string_lossy()
        );
        exit(1);
    }
    let src = std::fs::read_to_string(args.input_file).unwrap();

    let mut remc = Compiler::new(src);
    remc.build_ast();

    if args.ast {
        println!("{:#?}", remc.program);
    }

    let mut wat_path = PathBuf::from(&output_file);
    wat_path.set_extension("wat");
    let wat_filename = wat_path.to_string_lossy().into_owned();

    let mut wasm_path = PathBuf::from(&output_file);
    wasm_path.set_extension("wasm");
    let wasm_filename = wasm_path.to_string_lossy().into_owned();

    let mut backend = Backend::new(wat_filename, remc.program);
    backend.compile_program();

    let mut wat_text = String::new();
    backend.wat_file.read_to_string(&mut wat_text).unwrap();

    let wasm_bytes = wat::parse_str(wat_text).unwrap();
    std::fs::write(&wasm_filename, &wasm_bytes).unwrap();

    let engine = Engine::default();

    let module = Module::new(&engine, &wasm_bytes).expect("Failed to compile WASM module");
    let native_exe_bytes = module
        .serialize()
        .expect("Failed to serialize native machine code");

    write(&output_file, &native_exe_bytes).unwrap();

    if args.run {
        std::process::Command::new("wasmtime")
            .args(&["run", "--allow-precompiled", &output_file])
            .status()
            .expect("Failed to execute wasmtime runner");
    }
}
