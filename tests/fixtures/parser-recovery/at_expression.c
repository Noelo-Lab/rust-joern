int at_assignment(int x) { x=identifier@16; x++; return x; }
int at_declaration(int x) { int y=identifier@16; x++; return x; }
int at_return(int x) { return identifier@16; x++; }
int at_scalar_if(int x) { if(x) identifier@16; x++; return x; }
int at_condition(int x) { if(x@16) x++; return x; }
int at_braced_if(int x) { if(x) {identifier@16;} x++; return x; }
int at_register_expr(int x) { x=identifier@<eax>; x++; return x; }
int at_after(int x) { return x; }
