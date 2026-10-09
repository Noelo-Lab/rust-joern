char *gettext(char *);
void report_error(char *,int);
int work(int);
int quoted_backtick_call(int x){report_error(gettext("bad substitution: no closing "`" in %s"),0);work(x);return x;}
int quoted_backtick_assignment(int x){char *s="before "`" after";work(x);return x;}
int backtick_between_operators(int x){x=x`+1;work(x);return x;}
int backtick_call_argument(int x){x=work(`x);work(x);return x;}
int quoted_backtick_if_scalar(int x){if(x)report_error(gettext("before "`" after"),0);work(x);return x;}
int quoted_backtick_if_braced(int x){if(x){report_error(gettext("before "`" after"),0);work(x);}return x;}
int quoted_backtick_condition(int x){if(gettext("before "`" after"))work(x);return x;}
int quoted_backtick_while(int x){while(x)report_error(gettext("before "`" after"),0);work(x);return x;}
int quoted_backtick_do(int x){do report_error(gettext("before "`" after"),0);while(x);work(x);return x;}
int empty_cast_assignment(int x){char *v5;v5=(char *)/* unsupported instruction */;work(x);return x;}
int empty_cast_initializer(int x){char *v5=(char *)/* unsupported instruction */;work(x);return x;}
int empty_cast_standalone(int x){(char *)/* unsupported instruction */;work(x);return x;}
int empty_cast_return(int x){return (char *)/* unsupported instruction */;work(x);x++;return x;}
int empty_cast_if_scalar(int x){char *v5;if(x)v5=(char *)/* unsupported instruction */;work(x);return x;}
int empty_cast_if_braced(int x){char *v5;if(x){v5=(char *)/* unsupported instruction */;work(x);}return x;}
int empty_cast_while_scalar(int x){char *v5;while(x)v5=(char *)/* unsupported instruction */;work(x);return x;}
int empty_cast_while_braced(int x){char *v5;while(x){v5=(char *)/* unsupported instruction */;work(x);}return x;}
int empty_cast_do_scalar(int x){char *v5;do v5=(char *)/* unsupported instruction */;while(x);work(x);return x;}
int empty_cast_do_braced(int x){char *v5;do{v5=(char *)/* unsupported instruction */;work(x);}while(x);return x;}
int valid_adjacent_literals(int x){report_error(gettext("bad substitution: no closing "" in %s"),0);work(x);return x;}
int valid_escaped_backtick(int x){report_error(gettext("bad substitution: no closing \"`\" in %s"),0);work(x);return x;}
int valid_cast_assignment(int x){char *v5;v5=(char *)&x;work(x);return x;}
int valid_cast_if(int x){char *v5;if(x)v5=(char *)&x;work(x);return x;}
int after_all_controls(int x){work(x);return x;}
