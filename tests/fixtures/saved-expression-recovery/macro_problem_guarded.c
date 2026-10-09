#if 1
#define annotation(n)
int known(int x);
int plain_statement(int x) { x++; 1); x--; return x; }
int plain_assignment(int x) { x=1); x--; return x; }
int plain_initializer(int x) { int y=1); x--; return x; }
int plain_return(int x) { return 1); x++; return x; }
int plain_return_only(int x) { return 1); }
int plain_scalar_if(int x) { if(x) 1); x--; return x; }
int plain_scalar_while(int x) { while(x) 1); x--; return x; }
int plain_braced_if(int x) { if(x) { 1); x--; } return x; }
int plain_if_condition(int x) { if(1)) x++; x--; return x; }
int plain_while_condition(int x) { while(1)) x--; return x; }
int plain_logical_condition(int x) { if(x && 1)) known(x); return x; }
int plain_larger_expression(int x) { x=x+1); x--; return x; }
int plain_call_argument(int x) { known(1)); x--; return x; }
int macro_statement(int x) { x++; annotation(x,1); x--; return x; }
int macro_assignment(int x) { x=annotation(x,1); x--; return x; }
int macro_initializer(int x) { int y=annotation(x,1); x--; return x; }
int macro_return(int x) { return annotation(x,1); x++; return x; }
int macro_return_only(int x) { return annotation(x,1); }
int macro_scalar_if(int x) { if(x) annotation(x,1); x--; return x; }
int macro_scalar_while(int x) { while(x) annotation(x,1); x--; return x; }
int macro_braced_if(int x) { if(x) { annotation(x,1); x--; } return x; }
int macro_if_condition(int x) { if(annotation(x,1)) x++; x--; return x; }
int macro_while_condition(int x) { while(annotation(x,1)) x--; return x; }
int macro_logical_condition(int x) { if(x && annotation(x,1)) known(x); return x; }
int macro_larger_expression(int x) { x=x+annotation(x,1); x--; return x; }
int macro_call_argument(int x) { known(annotation(x,1)); x--; return x; }
int valid_grouped(int x) { x=(1); known((x)); if((x)) known(x); return x; }
int valid_empty_macro(int x) { annotation(x); x++; return x; }
int valid_empty_return(int x) { return annotation(x); }
int valid_empty_initializer(int x) { int y=annotation(x); return x; }
int valid_empty_argument(int x) { annotation(); x++; return x; }
int valid_macro_larger_expression(int x) { x=annotation(x)+1; return x; }
#endif
