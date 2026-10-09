int work(int);
int empty_if(int n){if()work(n);return n;}
int empty_while(int n){while()work(n);return n;}
int placeholder_condition(int n){if(?<=?)work(n);return n;}
int placeholder_assignment(int n){n=?;work(n);return n;}
int missing_lhs(int n){=n+1;work(n);return n;}
int missing_rhs(int n){n=;work(n);return n;}
int missing_initializer(int n){int x=;work(n);return n;}
int missing_binary_rhs(int n){n=work(n)+;work(n);return n;}
int missing_condition_operand(int n){if(n&&)work(n);return n;}
int missing_ternary_condition(int n){n=(?:n);work(n);return n;}
int valid_omitted_ternary(int n){return n?:work(n);}
int neutral_colon_assignment(int n){n=left:right/n;work(n);return n;}
int neutral_colon_return(int n){return left:right/n;}
int neutral_colon_initializer(int n){int x=left:right/n;work(n);return n;}
int neutral_colon_condition(int n){if(left:right)work(n);return n;}
int valid_label(int n){again:n=work(n);return n;}
int signed_shift(int n){n s>>=2;work(n);return n;}
int signed_multiply(int n){n=n*s result;work(n);return n;}
int missing_semicolon(int n){n=work(n)
work(n);return n;}
int missing_semicolon_problem(int n){unknown(<fake>)
work(n);return n;}
int bad_prefix_angle(int n){n=<0x12[is_4]|Stack bp-0x8>;work(n);return n;}
int after_bad_prefix_angle(int n){return work(n);}
