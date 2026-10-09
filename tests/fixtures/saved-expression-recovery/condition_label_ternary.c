int known(int);
struct Pair { int field; };
int comma_if(int x){if((,x))known(x);return x;}
int comma_nested_if(int x){if(x&&!(,x|| (,x+1)))known(x);return x;}
int comma_while(int x){while((,x))known(x);return x;}
int comma_statement(int x){x=(,known(x));known(x);return x;}
int comma_return(int x){return (,known(x));}
int comma_initializer(int x){int y=(,known(x));known(x);return x;}
int comma_scalar_if_body(int x){if(x)x=(,known(x));known(x);return x;}
int comma_braced_if_body(int x){if(x){x=(,known(x));known(x);}return x;}
int comma_valid_if(int x){if((known(x),x))known(x);return x;}
int comma_valid_statement(int x){x=(known(x),x+1);known(x);return x;}
int comma_valid_gnu(int x){if(({known(x);x;}))known(x);return x;}
int call_before_label(int x){known(x!=0)
again:known(x);return x;}
int empty_call_before_label(int x){known()
again:known(x);return x;}
int call_before_case(int x){switch(x){case 1:known(x!=0)
default:known(x);break;}return x;}
int empty_call_before_case(int x){switch(x){case 1:known()
default:known(x);break;}return x;}
int scalar_call_before_label(int x){if(x)known(x!=0)
again:known(x);return x;}
int braced_call_before_label(int x){if(x){known(x!=0)
again:known(x);}return x;}
int proper_call_before_label(int x){known(x!=0);again:known(x);return x;}
int proper_call_before_case(int x){switch(x){case 1:known(x!=0);default:known(x);break;}return x;}
int quoted_ternary_assignment(int x){char *p;p=x ? " rdomain "" : (char*)0;
void*const next=(void*)0;known(x);return x;}
int quoted_ternary_initializer(int x){char *p=x ? " rdomain "" : (char*)0;
void*const next=(void*)0;known(x);return x;}
int quoted_ternary_return(int x){return x ? " rdomain "" : (char*)0;
known(x);return x;}
int quoted_ternary_scalar_if(int x){char *p;if(x)p=x ? " rdomain "" : (char*)0;
void*const next=(void*)0;known(x);return x;}
int quoted_ternary_braced_if(int x){char *p;if(x){p=x ? " rdomain "" : (char*)0;
void*const next=(void*)0;known(x);}return x;}
int quoted_ternary_valid(int x){char*p=x ? " rdomain " "" : (char*)0;known(x);return x;}
int quoted_ternary_valid_character(int x){int y=x ? 'a' : 0;known(y);return x;}
int after_control(int x){known(x);return x;}
