#[derive(Clone, Debug, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
}

#[derive(Clone, Debug)]
pub struct Function {
    pub name: String,
    pub full_name: String,
    pub return_type: String,
    /// CDT's canonical function binding type can differ from METHOD_RETURN's
    /// declaration-specifier type, especially for typedefs and pointer returns.
    pub binding_return_type: String,
    pub signature: String,
    pub extern_c: bool,
    pub implicit_this: Option<String>,
    /// Lambda bodies are converted in their enclosing method's lexical scope.
    pub lambda_parent: Option<String>,
    pub implicit_fields: Vec<(String, String)>,
    pub member_cv_qualified: bool,
    /// CDT stores a lambda method separately from the enclosing method AST.
    pub lambda: bool,
    pub is_static: bool,
    /// Lambda bodies are converted in the enclosing lexical scope, after the
    /// lambda parameter scope has already been popped by c2cpg 4.0.150.
    pub inherited_bindings: Vec<(String, String)>,
    pub inherited_closures: Vec<(String, Closure)>,
    pub parameters: Vec<Parameter>,
    pub body: Stmt,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Parameter {
    pub name: String,
    pub type_name: String,
    pub function_pointer: bool,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct RetainedMacroCall {
    pub name: String,
    pub definition_span: Span,
    pub formal_arity: usize,
    pub parameter_count: usize,
}

#[derive(Clone, Debug)]
pub struct Declaration {
    pub name: String,
    pub type_name: String,
    pub initializer: Option<Expr>,
    pub dimensions: Vec<Expr>,
    /// CDT's function-declarator path cannot convert a C++ variable binding.
    pub problem: bool,
    /// A typedef declarator creates a type declaration, while its array bounds
    /// still participate in Joern's allocation lowering.
    pub is_typedef: bool,
    /// Initializer conversion reads the outer declarator's name, which is
    /// empty when the actual local name belongs to a parenthesized declarator.
    pub nested_declarator: bool,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Closure {
    pub full_name: String,
    /// The closure call signature uses CDT's deduced function type, while the
    /// detached method's signature uses ANY unless it has a trailing return.
    pub return_type: String,
    pub parameter_types: Vec<String>,
}

#[derive(Clone, Debug)]
pub enum ExprKind {
    Identifier(String),
    /// A type-id operand uses a decl-specifier identifier, never a method ref.
    TypeSpecifier {
        code: String,
        type_name: String,
    },
    Literal(String),
    Lambda(Closure),
    Unary {
        op: String,
        argument: Box<Expr>,
        postfix: bool,
    },
    Binary {
        op: String,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Conditional {
        condition: Box<Expr>,
        consequence: Box<Expr>,
        alternative: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        arguments: Vec<Expr>,
    },
    /// A bracketed identifier used as a call receiver, rather than a bare name.
    Bracketed(Box<Expr>),
    MacroCall {
        name: String,
        arguments: Vec<Expr>,
        expansion: Box<Expr>,
        arity: usize,
    },
    /// Generated macro syntax keeps its own CODE while sharing the call site.
    Generated {
        expression: Box<Expr>,
        code: String,
    },
    Member {
        base: Box<Expr>,
        name: String,
        indirect: bool,
    },
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
    },
    Cast {
        type_name: String,
        argument: Box<Expr>,
    },
    List(Vec<Expr>),
    /// An AST block used for declaration conditions and designator assignments.
    Block(Vec<Expr>),
    ArrayInitializer(Vec<Expr>),
    /// Inline assembly is an explicit UNKNOWN node in the CDT conversion.
    Assembly(String),
    /// A recognized CDT problem-expression recovery, retained as UNKNOWN.
    Problem(String),
    Statement(Box<Stmt>),
    Unknown(String),
}

#[derive(Clone, Debug)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum StmtKind {
    Block(Vec<Stmt>),
    /// Several sibling AST statements produced by one CDT recovery construct.
    Sequence(Vec<Stmt>),
    Expression(Expr),
    Declaration(Vec<Declaration>),
    If {
        condition: Expr,
        consequence: Box<Stmt>,
        alternative: Option<Box<Stmt>>,
    },
    While {
        condition: Expr,
        body: Box<Stmt>,
    },
    DoWhile {
        body: Box<Stmt>,
        condition: Expr,
    },
    For {
        initializer: Option<Box<Stmt>>,
        condition: Option<Expr>,
        update: Option<Expr>,
        body: Box<Stmt>,
    },
    RangeFor {
        declaration: Declaration,
        iterable: Expr,
        body: Box<Stmt>,
    },
    Switch {
        condition: Expr,
        body: Box<Stmt>,
    },
    Case(Option<Expr>),
    Label {
        name: String,
        statement: Box<Stmt>,
    },
    Goto(String),
    Break,
    Continue,
    Return(Option<Expr>),
    Try {
        body: Box<Stmt>,
        catches: Vec<Stmt>,
    },
    Throw(Option<Expr>),
    Empty,
    /// A recognized CDT problem statement, omitted by its AST conversion.
    Problem,
    Unknown(Vec<Expr>),
}

#[derive(Clone, Debug)]
pub struct ParseDiagnostic {
    pub message: String,
    pub span: Span,
}

#[derive(Clone, Debug, Default)]
pub struct TranslationUnit {
    pub functions: Vec<Function>,
    pub global_expressions: Vec<Expr>,
    pub retained_macro_calls: Vec<RetainedMacroCall>,
    pub diagnostics: Vec<ParseDiagnostic>,
}
