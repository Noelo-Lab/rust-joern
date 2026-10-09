int known(void);
int take(int x);
typedef long Scalar;
int signed_mul_initializer(int x,int y) { int z=x *s y; x++; return x; }
int signed_mul_alias_initializer(int x,int y) { Scalar z=x *s y; x++; return x; }
int signed_mul_for_initializer(int x,int y) { for(x=x *s y;x<3;x++) take(x); return x; }
int signed_mul_for_update(int x,int y) { for(x=0;x<3;x=x *s y) take(x); return x; }
int signed_mul_for_condition(int x,int y) { for(x=0;x *s y;x++) take(x); return x; }
int signed_mul_if_condition(int x,int y) { if(x *s y) take(x); return x; }
int signed_mul_scalar_body(int x,int y) { while(x) x=x *s y; x++; return x; }
int signed_mul_braced_body(int x,int y) { while(x) { x=x *s y; x++; } return x; }
int signed_wide_initializer(int x,int y) { int z=x *s128 y; x++; return x; }
int wide_initializer(int x,int y) { int z=x *128 y; x++; return x; }
int unsigned_shift_statement(int x) { x u>>=8; x++; return x; }
int unsigned_shift_for_initializer(int x) { for(x u>>=8;x;x--) take(x); return x; }
int unsigned_shift_for_update(int x) { for(;x;x u>>=8) take(x); return x; }
int unsigned_shift_for_condition(int x) { for(x=3;x u>>=8;x--) take(x); return x; }
int unsigned_shift_scalar_body(int x) { while(x) x u>>=8; x++; return x; }
int unsigned_shift_braced_body(int x) { while(x) { x u>>=8; x++; } return x; }
int unsigned_shift_return(int x) { return x u>>=8; x++; return x; }
int separator_empty_assignment(int x) { known() x=take(x); x++; return x; }
int separator_empty_call(int x) { known() take(x); x++; return x; }
int separator_empty_declaration(int x) { known() int y=x; return y; }
int separator_empty_if(int x) { known() if(x) take(x); return x; }
int separator_empty_while(int x) { known() while(x) take(x); return x; }
int separator_empty_block(int x) { known() { take(x); } return x; }
int separator_empty_return(int x) { known() return x; }
int separator_empty_increment(int x) { known() x++; return x; }
int separator_unknown_empty_assignment(int x) { mystery() x=take(x); return x; }
int valid_multiply_initializer(int x,int y) { int z=x*y; return z; }
int valid_multiply_for(int x,int y) { for(x=x*y;x<3;x=x*y) take(x); return x; }
int valid_shift_statement(int x) { x>>=8; x++; return x; }
int valid_shift_for(int x) { for(;x;x>>=8) take(x); return x; }
int valid_postincrement_for(int x) { for(x=0;x<3;x++) take(x); return x; }
int valid_empty_call_separators(int x) { known(); x=take(x); known(); if(x)take(x); return x; }
int valid_empty_call_block(int x) { known(); {take(x);} return x; }
int valid_cpp_alternative_tokens(int x,int y) { if(x and not y) x or_eq y; return x; }
