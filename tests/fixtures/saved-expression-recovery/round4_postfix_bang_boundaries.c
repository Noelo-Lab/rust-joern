typedef int T;
int known(int x);
int other(int x);
struct Receiver{int (*callback)(int);};
int bare_bang_statement(int x,int r6){x=r6!;x++;return x;}
int bare_bang_return(int x,int r6){return r6!;}
int bare_bang_initializer(int x,int r6){int y=r6!;return x;}
int bare_bang_condition(int x,int r6){if(r6!)x++;return x;}
int bare_bang_scalar_if(int x,int r6){if(x)x=r6!;return x;}
int bare_bang_braced_if(int x,int r6){if(x){x=r6!;}return x;}
int bare_bang_scalar_while(int x,int r6){while(x)x=r6!;return x;}
int bare_bang_braced_while(int x,int r6){while(x){x=r6!;}return x;}
int grouped_bang_statement(int x,int r6){x=(r6!);x++;return x;}
int grouped_bang_return(int x,int r6){return (r6!);}
int grouped_bang_initializer(int x,int r6){int y=(r6!);return x;}
int grouped_bang_condition(int x,int r6){if((r6!))x++;return x;}
int grouped_bang_scalar_if(int x,int r6){if(x)x=(r6!);return x;}
int grouped_bang_braced_if(int x,int r6){if(x){x=(r6!);}return x;}
int grouped_bang_scalar_while(int x,int r6){while(x)x=(r6!);return x;}
int grouped_bang_braced_while(int x,int r6){while(x){x=(r6!);}return x;}
int deref_bang_statement(int x,int r6){x=*(r6!);x++;return x;}
int deref_bang_return(int x,int r6){return *(r6!);}
int deref_bang_initializer(int x,int r6){int y=*(r6!);return x;}
int deref_bang_condition(int x,int r6){if(*(r6!))x++;return x;}
int deref_bang_scalar_if(int x,int r6){if(x)x=*(r6!);return x;}
int deref_bang_braced_if(int x,int r6){if(x){x=*(r6!);}return x;}
int deref_bang_scalar_while(int x,int r6){while(x)x=*(r6!);return x;}
int deref_bang_braced_while(int x,int r6){while(x){x=*(r6!);}return x;}
int binary_bang_statement(int x,int r6){x=*((r6! + 4));x++;return x;}
int binary_bang_return(int x,int r6){return *((r6! + 4));}
int binary_bang_initializer(int x,int r6){int y=*((r6! + 4));return x;}
int binary_bang_condition(int x,int r6){if(*((r6! + 4)))x++;return x;}
int binary_bang_scalar_if(int x,int r6){if(x)x=*((r6! + 4));return x;}
int binary_bang_braced_if(int x,int r6){if(x){x=*((r6! + 4));}return x;}
int binary_bang_scalar_while(int x,int r6){while(x)x=*((r6! + 4));return x;}
int binary_bang_braced_while(int x,int r6){while(x){x=*((r6! + 4));}return x;}
int nested_bang_statement(int x,int r6){x=*(((r6! + 4)));x++;return x;}
int nested_bang_return(int x,int r6){return *(((r6! + 4)));}
int nested_bang_initializer(int x,int r6){int y=*(((r6! + 4)));return x;}
int nested_bang_condition(int x,int r6){if(*(((r6! + 4))))x++;return x;}
int nested_bang_scalar_if(int x,int r6){if(x)x=*(((r6! + 4)));return x;}
int nested_bang_braced_if(int x,int r6){if(x){x=*(((r6! + 4)));}return x;}
int nested_bang_scalar_while(int x,int r6){while(x)x=*(((r6! + 4)));return x;}
int nested_bang_braced_while(int x,int r6){while(x){x=*(((r6! + 4)));}return x;}
int call_suffix_bang_statement(int x,int r6){x=*((r6! + known(x)));x++;return x;}
int call_suffix_bang_return(int x,int r6){return *((r6! + known(x)));}
int call_suffix_bang_initializer(int x,int r6){int y=*((r6! + known(x)));return x;}
int call_suffix_bang_condition(int x,int r6){if(*((r6! + known(x))))x++;return x;}
int call_suffix_bang_scalar_if(int x,int r6){if(x)x=*((r6! + known(x)));return x;}
int call_suffix_bang_braced_if(int x,int r6){if(x){x=*((r6! + known(x)));}return x;}
int call_suffix_bang_scalar_while(int x,int r6){while(x)x=*((r6! + known(x)));return x;}
int call_suffix_bang_braced_while(int x,int r6){while(x){x=*((r6! + known(x)));}return x;}
int grouped_adjacent_statement(int x,int r6){x=(known(x)+2) CONCAT (known(r6)+3);x++;return x;}
int grouped_adjacent_return(int x,int r6){return (known(x)+2) CONCAT (known(r6)+3);}
int grouped_adjacent_initializer(int x,int r6){int y=(known(x)+2) CONCAT (known(r6)+3);return x;}
int grouped_adjacent_condition(int x,int r6){if((known(x)+2) CONCAT (known(r6)+3))x++;return x;}
int grouped_adjacent_scalar_if(int x,int r6){if(x)x=(known(x)+2) CONCAT (known(r6)+3);return x;}
int grouped_adjacent_braced_if(int x,int r6){if(x){x=(known(x)+2) CONCAT (known(r6)+3);}return x;}
int grouped_adjacent_scalar_while(int x,int r6){while(x)x=(known(x)+2) CONCAT (known(r6)+3);return x;}
int grouped_adjacent_braced_while(int x,int r6){while(x){x=(known(x)+2) CONCAT (known(r6)+3);}return x;}
int lhs_bare_statement(int r1,int ip){*(ip!)=r1;r1++;return r1;}
int lhs_bare_condition(int r1,int ip){if((*(ip!)=r1))r1++;return r1;}
int lhs_bare_scalar_if(int r1,int ip){if(r1)*(ip!)=r1;return r1;}
int lhs_bare_braced_if(int r1,int ip){if(r1){*(ip!)=r1;}return r1;}
int lhs_binary_statement(int r1,int ip){*((ip! + 4))=r1;r1++;return r1;}
int lhs_binary_condition(int r1,int ip){if((*((ip! + 4))=r1))r1++;return r1;}
int lhs_binary_scalar_if(int r1,int ip){if(r1)*((ip! + 4))=r1;return r1;}
int lhs_binary_braced_if(int r1,int ip){if(r1){*((ip! + 4))=r1;}return r1;}
int lhs_nested_statement(int r1,int ip){*(((ip! + 4)))=r1;r1++;return r1;}
int lhs_nested_condition(int r1,int ip){if((*(((ip! + 4)))=r1))r1++;return r1;}
int lhs_nested_scalar_if(int r1,int ip){if(r1)*(((ip! + 4)))=r1;return r1;}
int lhs_nested_braced_if(int r1,int ip){if(r1){*(((ip! + 4)))=r1;}return r1;}
int positive_not_tilde(int x){x=!x;x=~x;return x;}
int positive_group_not(int x,int r6){return !(r6+4)+x;}
int positive_explicit_cast(int x){return (T)!x;}
int positive_unknown_cast(int x){return (UnknownAlias)!x;}
int positive_object_cast(int T,int x){return (T)!x;}
int positive_cast_identifier(int x){return (T)x;}
int positive_sizeof(int x){return sizeof(T)+sizeof(x);}
int positive_group_binary(int x,int r6){return (known(x)+2)*(known(r6)+3);}
int positive_pointer_call(void *p,int x){return ((int(*)(int))p)(x);}
int positive_member_call(struct Receiver *r,int x){return r->callback(x);}
int positive_conditional_call(int x){return (x?known:other)(x);}
int positive_lhs_group(int *ip,int r1){*((ip+4))=r1;return r1;}
int after_control(int x){return x;}
