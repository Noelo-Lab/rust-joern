void inferred_parameter(InferredU value);
typedef int AliasT;
int unknown_post_inc(int x){(UnknownT)++;return x;}
int unknown_deref_post_inc(int x){*(UnknownT)++;return x;}
int unknown_post_dec(int x){(UnknownT)--;return x;}
int unknown_deref_post_dec(int x){*(UnknownT)--;return x;}
int inferred_post_inc(int x){(InferredU)++;return x;}
int inferred_deref_post_inc(int x){*(InferredU)++;return x;}
int inferred_post_dec(int x){(InferredU)--;return x;}
int inferred_deref_post_dec(int x){*(InferredU)--;return x;}
int explicit_post_inc(int x){(AliasT)++;return x;}
int explicit_deref_post_inc(int x){*(AliasT)++;return x;}
int explicit_post_dec(int x){(AliasT)--;return x;}
int explicit_deref_post_dec(int x){*(AliasT)--;return x;}
int shadow_post_inc(int AliasT,int x){(AliasT)++;return x;}
int shadow_deref_post_inc(int AliasT,int x){*(AliasT)++;return x;}
int shadow_post_dec(int AliasT,int x){(AliasT)--;return x;}
int shadow_deref_post_dec(int AliasT,int x){*(AliasT)--;return x;}
int object_post_inc(int value,int x){(value)++;return x;}
int object_deref_post_inc(int value,int x){*(value)++;return x;}
int object_post_dec(int value,int x){(value)--;return x;}
int object_deref_post_dec(int value,int x){*(value)--;return x;}
int postfix_bang_statement(int x,int r6){x=*(r6!);x++;return x;}
int postfix_bang_return(int x,int r6){return *(r6!);}
int postfix_bang_initializer(int x,int r6){int y=*(r6!);return x;}
int postfix_bang_condition(int x,int r6){if(*(r6!))x++;return x;}
int postfix_bang_scalar_if(int x,int r6){if(x)x=*(r6!);return x;}
int postfix_bang_braced_if(int x,int r6){if(x){x=*(r6!);}return x;}
int postfix_bang_scalar_while(int x,int r6){while(x)x=*(r6!);return x;}
int postfix_bang_braced_while(int x,int r6){while(x){x=*(r6!);}return x;}
int adjacent_group_statement(int x,int a,int b,int c,int d){x=(a CONCAT b)^(c CONCAT d);x++;return x;}
int adjacent_group_return(int x,int a,int b,int c,int d){return (a CONCAT b)^(c CONCAT d);}
int adjacent_group_initializer(int x,int a,int b,int c,int d){int y=(a CONCAT b)^(c CONCAT d);return x;}
int adjacent_group_condition(int x,int a,int b,int c,int d){if((a CONCAT b)^(c CONCAT d))x++;return x;}
int adjacent_group_scalar_if(int x,int a,int b,int c,int d){if(x)x=(a CONCAT b)^(c CONCAT d);return x;}
int adjacent_group_braced_if(int x,int a,int b,int c,int d){if(x){x=(a CONCAT b)^(c CONCAT d);}return x;}
int adjacent_group_scalar_while(int x,int a,int b,int c,int d){while(x)x=(a CONCAT b)^(c CONCAT d);return x;}
int adjacent_group_braced_while(int x,int a,int b,int c,int d){while(x){x=(a CONCAT b)^(c CONCAT d);}return x;}
int valid_not_complement(int x){x=!x;x=~x;return x;}
int valid_group_multiplication(int a,int b,int c,int d){return (a*b)^(c*d);}
int valid_group_calls(int x){return (known(x))^(known(x+1));}
int valid_deref_postfix(int *p){*p++;*(p)++;return *p;}
int valid_cast_increment(int x){return (AliasT)++x;}
int valid_unknown_cast_increment(int x){return (UnknownT)++x;}
int unknown_postfix_return(int x){return (UnknownT)++;}
int unknown_deref_postfix_return(int x){return *(UnknownT)++;}
int inferred_postfix_return(int x){return (InferredU)++;}
int explicit_postfix_return(int x){return (AliasT)++;}
int shadow_postfix_return(int AliasT,int x){return (AliasT)++;}
int object_postfix_return(int value,int x){return (value)++;}
int ungrouped_postfix_suffix(int x,int suffix){x++suffix;return x;}
int ungrouped_postfix_suffix_return(int x,int suffix){return x++suffix;}
int after_control(int x){return x;}
