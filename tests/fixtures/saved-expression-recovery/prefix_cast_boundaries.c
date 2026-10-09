void inferred_parameter(InferredU value);
typedef int AliasT;
int unknown_inc_assign(int x){int y=0;y=(UnknownT)++x;return y;}
int unknown_inc_return(int x){return (UnknownT)++x;}
int unknown_inc_init(int x){int y=(UnknownT)++x;return y;}
int unknown_inc_condition(int x){if((UnknownT)++x)x+=2;return x;}
int unknown_dec_assign(int x){int y=0;y=(UnknownT)--x;return y;}
int unknown_dec_return(int x){return (UnknownT)--x;}
int unknown_dec_init(int x){int y=(UnknownT)--x;return y;}
int unknown_dec_condition(int x){if((UnknownT)--x)x+=2;return x;}
int inferred_inc_assign(int x){int y=0;y=(InferredU)++x;return y;}
int inferred_inc_return(int x){return (InferredU)++x;}
int inferred_inc_init(int x){int y=(InferredU)++x;return y;}
int inferred_inc_condition(int x){if((InferredU)++x)x+=2;return x;}
int inferred_dec_assign(int x){int y=0;y=(InferredU)--x;return y;}
int inferred_dec_return(int x){return (InferredU)--x;}
int inferred_dec_init(int x){int y=(InferredU)--x;return y;}
int inferred_dec_condition(int x){if((InferredU)--x)x+=2;return x;}
int explicit_inc_assign(int x){int y=0;y=(AliasT)++x;return y;}
int explicit_inc_return(int x){return (AliasT)++x;}
int explicit_inc_init(int x){int y=(AliasT)++x;return y;}
int explicit_inc_condition(int x){if((AliasT)++x)x+=2;return x;}
int explicit_dec_assign(int x){int y=0;y=(AliasT)--x;return y;}
int explicit_dec_return(int x){return (AliasT)--x;}
int explicit_dec_init(int x){int y=(AliasT)--x;return y;}
int explicit_dec_condition(int x){if((AliasT)--x)x+=2;return x;}
int shadow_inc_assign(int AliasT,int x){int y=0;y=(AliasT)++x;return y;}
int shadow_inc_return(int AliasT,int x){return (AliasT)++x;}
int shadow_inc_init(int AliasT,int x){int y=(AliasT)++x;return y;}
int shadow_inc_condition(int AliasT,int x){if((AliasT)++x)x+=2;return x;}
int shadow_dec_assign(int AliasT,int x){int y=0;y=(AliasT)--x;return y;}
int shadow_dec_return(int AliasT,int x){return (AliasT)--x;}
int shadow_dec_init(int AliasT,int x){int y=(AliasT)--x;return y;}
int shadow_dec_condition(int AliasT,int x){if((AliasT)--x)x+=2;return x;}
int object_inc_assign(int value,int x){int y=0;y=(value)++x;return y;}
int object_inc_return(int value,int x){return (value)++x;}
int object_inc_init(int value,int x){int y=(value)++x;return y;}
int object_inc_condition(int value,int x){if((value)++x)x+=2;return x;}
int object_dec_assign(int value,int x){int y=0;y=(value)--x;return y;}
int object_dec_return(int value,int x){return (value)--x;}
int object_dec_init(int value,int x){int y=(value)--x;return y;}
int object_dec_condition(int value,int x){if((value)--x)x+=2;return x;}
int valid_prefix(int x){x=++x;x=--x;return x;}
int valid_postfix(int x){x=x++;x=x--;return x;}
int valid_grouped_postfix(int x){(x)++;(x)--;return x;}
int valid_cast_prefix(int x){int y=(AliasT)++x;y=(AliasT)--x;return y;}
int valid_cast_postfix(int x){int y=(AliasT)x++;y=(AliasT)x--;return y;}
int unknown_return_followup(int x){return (UnknownT)++x;x+=2;return x;}
int shadow_return_followup(int AliasT,int x){return (AliasT)++x;x+=2;return x;}
int object_return_followup(int value,int x){return (value)++x;x+=2;return x;}
int malformed_postfix_assign(int value,int suffix){int y=0;y=(value)++suffix;return y;}
int malformed_postfix_return(int value,int suffix){return (value)--suffix;}
int malformed_postfix_init(int value,int suffix){int y=(value)++suffix;return y;}
int malformed_postfix_condition(int value,int suffix){if((value)--suffix)value+=2;return value;}
int after_control(int x){return x;}
