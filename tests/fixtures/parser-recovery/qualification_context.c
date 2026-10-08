int stream;
int foo(int x);
int local_qualified(int x) { int y=::stream; return y; }
int local_qualified_condition(int x) { int y=(::stream && x); return y; }
int return_qualified_condition(int x) { return x || ::stream; }
int argument_qualified(int x) { foo(::stream); return x; }
int for_qualified_init(int x) { for (x=::stream; x<4; ++x) foo(x); return x; }
int for_qualified_condition(int x) { for (x=0; x && ::stream; ++x) foo(x); return x; }
int for_qualified_update(int x) { for (x=0; x<4; ::stream++) foo(x); return x; }
int statement_expression_return(int x) { return ({ ::stream=x; x; }); }
int statement_expression_if(int x) { if (({ ::stream=x; x; })) return 1; return 0; }
