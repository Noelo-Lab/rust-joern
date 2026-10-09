int work(int);
typedef int i;
int valid_do(int x){do{work(x);x--;}while(x);work(x);return x;}
int missing_do_while_assignment(int x){do{work(x);}x=work(x);x++;return x;}
int missing_do_while_return(int x){do{work(x);}return work(x);}
int missing_do_while_label(int x){do{work(x);}again:work(x);x++;return x;}
int missing_do_while_if(int x){do{work(x);}if(x)work(x);x++;return x;}
int missing_outer_do_while(int x){do{do{work(x);}while(x);}x=work(x);work(x);return x;}
int do_body_standalone_while(int x){do{if(x)work(x);else work(x+1);while(1);}x=work(x);work(x);return x;}
int missing_do_condition(int x){do{work(x);}while;work(x);return x;}
int missing_do_semicolon(int x){do{work(x);}while(x)work(x);x++;return x;}
int valid_for_empty_init(int n){int k=0;for(;k<((n<<2)+3);k=(n>>k)&7)work(k);return k;}
int valid_for_shadowed_type(int n){int i=0;for(;i<((n<<2)+3);i=(n>>i)&7)work(i);return i;}
int for_missing_separator(int x){for(x=0;x<3 x++)work(x);x++;return x;}
int for_missing_update(int x){for(x=0;x<3)work(x);x++;return x;}
int assignment_unclosed_string(int x){x="broken;
work(x);x++;return x;}
int assignment_unclosed_char(int x){x='broken;
work(x);x++;return x;}
int return_unclosed_string(int x){return "broken;
work(x);x++;return x;}
int return_unclosed_char(int x){return 'broken;
work(x);x++;return x;}
int label_unclosed_string(int x){goto again;again:x="broken;
work(x);x++;return x;}
int scalar_if_unclosed_string(int x){if(x)x="broken;
work(x);x++;return x;}
int scalar_while_unclosed_char(int x){while(x)x='broken;
work(x);x++;return x;}
int do_unclosed_string(int x){do x="broken;
work(x);while(x);x++;return x;}
int declaration_unclosed_string(int x){char *s="broken;
work(x);x++;return x;}
int after_all_boundaries(int x){work(x);return x;}
