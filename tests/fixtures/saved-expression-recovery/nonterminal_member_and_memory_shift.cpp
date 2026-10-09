int known();
struct Obj{int field;};
struct Pair{int x;};
int call_empty_before_member_compound(int x,struct Obj *p,int *q){known()
p->field=x;return x;}
int call_empty_before_member_scalar_if(int x,struct Obj *p,int *q){if(x)known()
p->field=x;return x;}
int call_empty_before_member_braced_if(int x,struct Obj *p,int *q){if(x){known()
p->field=x;}return x;}
int call_empty_before_member_scalar_while(int x,struct Obj *p,int *q){while(x)known()
p->field=x;return x;}
int call_empty_before_member_braced_while(int x,struct Obj *p,int *q){while(x){known()
p->field=x;}return x;}
int call_empty_before_member_proper(int x,struct Obj *p,int *q){known();
p->field=x;return x;}
int call_empty_before_deref_compound(int x,struct Obj *p,int *q){known()
*q=x;return x;}
int call_empty_before_deref_scalar_if(int x,struct Obj *p,int *q){if(x)known()
*q=x;return x;}
int call_empty_before_deref_braced_if(int x,struct Obj *p,int *q){if(x){known()
*q=x;}return x;}
int call_empty_before_deref_scalar_while(int x,struct Obj *p,int *q){while(x)known()
*q=x;return x;}
int call_empty_before_deref_braced_while(int x,struct Obj *p,int *q){while(x){known()
*q=x;}return x;}
int call_empty_before_deref_proper(int x,struct Obj *p,int *q){known();
*q=x;return x;}
int member_before_call_empty_compound(int x,struct Obj *p,int *q){p->field
known();return x;}
int member_before_call_empty_scalar_if(int x,struct Obj *p,int *q){if(x)p->field
known();return x;}
int member_before_call_empty_braced_if(int x,struct Obj *p,int *q){if(x){p->field
known();}return x;}
int member_before_call_empty_scalar_while(int x,struct Obj *p,int *q){while(x)p->field
known();return x;}
int member_before_call_empty_braced_while(int x,struct Obj *p,int *q){while(x){p->field
known();}return x;}
int member_before_call_empty_proper(int x,struct Obj *p,int *q){p->field;
known();return x;}
int call_plain_before_member_compound(int x,struct Obj *p,int *q){known(x)
p->field=x;return x;}
int call_plain_before_member_scalar_if(int x,struct Obj *p,int *q){if(x)known(x)
p->field=x;return x;}
int call_plain_before_member_braced_if(int x,struct Obj *p,int *q){if(x){known(x)
p->field=x;}return x;}
int call_plain_before_member_scalar_while(int x,struct Obj *p,int *q){while(x)known(x)
p->field=x;return x;}
int call_plain_before_member_braced_while(int x,struct Obj *p,int *q){while(x){known(x)
p->field=x;}return x;}
int call_plain_before_member_proper(int x,struct Obj *p,int *q){known(x);
p->field=x;return x;}
int call_plain_before_deref_compound(int x,struct Obj *p,int *q){known(x)
*q=x;return x;}
int call_plain_before_deref_scalar_if(int x,struct Obj *p,int *q){if(x)known(x)
*q=x;return x;}
int call_plain_before_deref_braced_if(int x,struct Obj *p,int *q){if(x){known(x)
*q=x;}return x;}
int call_plain_before_deref_scalar_while(int x,struct Obj *p,int *q){while(x)known(x)
*q=x;return x;}
int call_plain_before_deref_braced_while(int x,struct Obj *p,int *q){while(x){known(x)
*q=x;}return x;}
int call_plain_before_deref_proper(int x,struct Obj *p,int *q){known(x);
*q=x;return x;}
int member_before_call_plain_compound(int x,struct Obj *p,int *q){p->field
known(x);return x;}
int member_before_call_plain_scalar_if(int x,struct Obj *p,int *q){if(x)p->field
known(x);return x;}
int member_before_call_plain_braced_if(int x,struct Obj *p,int *q){if(x){p->field
known(x);}return x;}
int member_before_call_plain_scalar_while(int x,struct Obj *p,int *q){while(x)p->field
known(x);return x;}
int member_before_call_plain_braced_while(int x,struct Obj *p,int *q){while(x){p->field
known(x);}return x;}
int member_before_call_plain_proper(int x,struct Obj *p,int *q){p->field;
known(x);return x;}
int call_comparison_before_member_compound(int x,struct Obj *p,int *q){known(x!=0)
p->field=x;return x;}
int call_comparison_before_member_scalar_if(int x,struct Obj *p,int *q){if(x)known(x!=0)
p->field=x;return x;}
int call_comparison_before_member_braced_if(int x,struct Obj *p,int *q){if(x){known(x!=0)
p->field=x;}return x;}
int call_comparison_before_member_scalar_while(int x,struct Obj *p,int *q){while(x)known(x!=0)
p->field=x;return x;}
int call_comparison_before_member_braced_while(int x,struct Obj *p,int *q){while(x){known(x!=0)
p->field=x;}return x;}
int call_comparison_before_member_proper(int x,struct Obj *p,int *q){known(x!=0);
p->field=x;return x;}
int call_comparison_before_deref_compound(int x,struct Obj *p,int *q){known(x!=0)
*q=x;return x;}
int call_comparison_before_deref_scalar_if(int x,struct Obj *p,int *q){if(x)known(x!=0)
*q=x;return x;}
int call_comparison_before_deref_braced_if(int x,struct Obj *p,int *q){if(x){known(x!=0)
*q=x;}return x;}
int call_comparison_before_deref_scalar_while(int x,struct Obj *p,int *q){while(x)known(x!=0)
*q=x;return x;}
int call_comparison_before_deref_braced_while(int x,struct Obj *p,int *q){while(x){known(x!=0)
*q=x;}return x;}
int call_comparison_before_deref_proper(int x,struct Obj *p,int *q){known(x!=0);
*q=x;return x;}
int member_before_call_comparison_compound(int x,struct Obj *p,int *q){p->field
known(x!=0);return x;}
int member_before_call_comparison_scalar_if(int x,struct Obj *p,int *q){if(x)p->field
known(x!=0);return x;}
int member_before_call_comparison_braced_if(int x,struct Obj *p,int *q){if(x){p->field
known(x!=0);}return x;}
int member_before_call_comparison_scalar_while(int x,struct Obj *p,int *q){while(x)p->field
known(x!=0);return x;}
int member_before_call_comparison_braced_while(int x,struct Obj *p,int *q){while(x){p->field
known(x!=0);}return x;}
int member_before_call_comparison_proper(int x,struct Obj *p,int *q){p->field;
known(x!=0);return x;}
int unbound_before_member(int x,struct Obj *p,int *q){unbound(x)
p->field=x;return x;}
int unbound_before_deref(int x,struct Obj *p,int *q){unbound(x!=0)
*q=x;return x;}
int positive_call_member_if(int x,struct Obj *p,int *q){if(x)known(x);p->field=x;return x;}
int positive_call_deref_if(int x,struct Obj *p,int *q){if(x)known(x!=0);*q=x;return x;}
int positive_member_call_if(int x,struct Obj *p,int *q){if(x)p->field;known(x);return x;}
int memory_u_suffix_statement(int x,struct Obj *p,int *q){*(q+1) u>>=8;return x;}
int memory_u_suffix_return(int x,struct Obj *p,int *q){return *(q+1) u>>=8;}
int memory_u_suffix_initializer(int x,struct Obj *p,int *q){int y=*(q+1) u>>=8;return x;}
int memory_u_suffix_condition(int x,struct Obj *p,int *q){if(*(q+1) u>>=8)x++;return x;}
int memory_u_suffix_scalar_if(int x,struct Obj *p,int *q){if(x)*(q+1) u>>=8;return x;}
int memory_u_suffix_braced_if(int x,struct Obj *p,int *q){if(x){*(q+1) u>>=8;}return x;}
int memory_u_suffix_scalar_while(int x,struct Obj *p,int *q){while(x)*(q+1) u>>=8;return x;}
int memory_u_suffix_braced_while(int x,struct Obj *p,int *q){while(x){*(q+1) u>>=8;}return x;}
int memory_u_suffix_scalar_do(int x,struct Obj *p,int *q){do *(q+1) u>>=8;while(x);return x;}
int memory_u_suffix_braced_do(int x,struct Obj *p,int *q){do{*(q+1) u>>=8;}while(x);return x;}
int memory_u_suffix_for_init(int x,struct Obj *p,int *q){for(*(q+1) u>>=8;x;x--)x++;return x;}
int memory_u_suffix_for_condition(int x,struct Obj *p,int *q){for(;*(q+1) u>>=8;)x++;return x;}
int memory_u_suffix_for_update(int x,struct Obj *p,int *q){for(;x;*(q+1) u>>=8)x++;return x;}
int memory_u_suffix_for_scalar_body(int x,struct Obj *p,int *q){for(;x;x--)*(q+1) u>>=8;return x;}
int memory_u_suffix_for_braced_body(int x,struct Obj *p,int *q){for(;x;x--){*(q+1) u>>=8;}return x;}
int memory_s_suffix_statement(int x,struct Obj *p,int *q){*(q+1) s>>=8;return x;}
int memory_s_suffix_return(int x,struct Obj *p,int *q){return *(q+1) s>>=8;}
int memory_s_suffix_initializer(int x,struct Obj *p,int *q){int y=*(q+1) s>>=8;return x;}
int memory_s_suffix_condition(int x,struct Obj *p,int *q){if(*(q+1) s>>=8)x++;return x;}
int memory_s_suffix_scalar_if(int x,struct Obj *p,int *q){if(x)*(q+1) s>>=8;return x;}
int memory_s_suffix_braced_if(int x,struct Obj *p,int *q){if(x){*(q+1) s>>=8;}return x;}
int memory_s_suffix_scalar_while(int x,struct Obj *p,int *q){while(x)*(q+1) s>>=8;return x;}
int memory_s_suffix_braced_while(int x,struct Obj *p,int *q){while(x){*(q+1) s>>=8;}return x;}
int memory_s_suffix_scalar_do(int x,struct Obj *p,int *q){do *(q+1) s>>=8;while(x);return x;}
int memory_s_suffix_braced_do(int x,struct Obj *p,int *q){do{*(q+1) s>>=8;}while(x);return x;}
int memory_s_suffix_for_init(int x,struct Obj *p,int *q){for(*(q+1) s>>=8;x;x--)x++;return x;}
int memory_s_suffix_for_condition(int x,struct Obj *p,int *q){for(;*(q+1) s>>=8;)x++;return x;}
int memory_s_suffix_for_update(int x,struct Obj *p,int *q){for(;x;*(q+1) s>>=8)x++;return x;}
int memory_s_suffix_for_scalar_body(int x,struct Obj *p,int *q){for(;x;x--)*(q+1) s>>=8;return x;}
int memory_s_suffix_for_braced_body(int x,struct Obj *p,int *q){for(;x;x--){*(q+1) s>>=8;}return x;}
int memory_identifier_suffix_statement(int x,struct Obj *p,int *q){*(q+1) marker>>=8;return x;}
int memory_identifier_suffix_return(int x,struct Obj *p,int *q){return *(q+1) marker>>=8;}
int memory_identifier_suffix_condition(int x,struct Obj *p,int *q){if(*(q+1) marker>>=8)x++;return x;}
int memory_identifier_suffix_scalar_if(int x,struct Obj *p,int *q){if(x)*(q+1) marker>>=8;return x;}
int memory_identifier_suffix_braced_if(int x,struct Obj *p,int *q){if(x){*(q+1) marker>>=8;}return x;}
int positive_memory_shift_statement(int x,struct Obj *p,int *q){*(q+1)>>=8;return x;}
int positive_memory_shift_return(int x,struct Obj *p,int *q){return *(q+1)>>=8;}
int positive_memory_shift_initializer(int x,struct Obj *p,int *q){int y=*(q+1)>>=8;return x;}
int positive_memory_shift_condition(int x,struct Obj *p,int *q){if(*(q+1)>>=8)x++;return x;}
int positive_memory_shift_scalar_if(int x,struct Obj *p,int *q){if(x)*(q+1)>>=8;return x;}
int positive_memory_shift_braced_if(int x,struct Obj *p,int *q){if(x){*(q+1)>>=8;}return x;}
int positive_memory_shift_scalar_while(int x,struct Obj *p,int *q){while(x)*(q+1)>>=8;return x;}
int positive_memory_shift_braced_while(int x,struct Obj *p,int *q){while(x){*(q+1)>>=8;}return x;}
int positive_memory_shift_scalar_do(int x,struct Obj *p,int *q){do *(q+1)>>=8;while(x);return x;}
int positive_memory_shift_braced_do(int x,struct Obj *p,int *q){do{*(q+1)>>=8;}while(x);return x;}
int positive_memory_shift_for_init(int x,struct Obj *p,int *q){for(*(q+1)>>=8;x;x--)x++;return x;}
int positive_memory_shift_for_update(int x,struct Obj *p,int *q){for(;x;*(q+1)>>=8)x++;return x;}
int positive_memory_shift_for_scalar_body(int x,struct Obj *p,int *q){for(;x;x--)*(q+1)>>=8;return x;}
int positive_memory_shift_for_braced_body(int x,struct Obj *p,int *q){for(;x;x--){*(q+1)>>=8;}return x;}
int positive_integer_shift(int x,struct Obj *p,int *q){x>>=8;return x;}
int brace_literal_condition(int x,int y){if(x || !({0} < y))x++;return x;}
int positive_compound_literal_condition(int x,int y){if(x || !((struct Pair){0}.x < y))x++;return x;}
int missing_operand_before_comma(int x,int v,int y){if(!x || (v=x+/*unsupported*/,y>v))x++;return x;}
int positive_operand_before_comma(int x,int v,int y){if(!x || (v=x+1,y>v))x++;return x;}
int malformed_const_pointer(int v){v=const void *);return v;}
int positive_const_pointer(int v){v=(int)(const void *)0;return v;}
int after_control(int x){return x;}
