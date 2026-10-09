int work(int);
int assignment_string_no_semicolon(int x){x="broken
work(x);x++;return x;}
int assignment_char_no_semicolon(int x){x='broken
work(x);x++;return x;}
int compound_assignment_string(int x){x+="broken;
work(x);x++;return x;}
int assignment_followed_assignment(int x){x="broken;
x=work(x);x++;return x;}
int declaration_char_no_semicolon(int x){char c='broken
work(x);x++;return x;}
int braced_if_string(int x){if(x){x="broken;
work(x);}x++;return x;}
int braced_while_char(int x){while(x){x='broken;
work(x);}x++;return x;}
int braced_do_string(int x){do{x="broken;
work(x);}while(x);x++;return x;}
int return_string_no_semicolon(int x){return "broken
work(x);x++;return x;}
int label_char_no_semicolon(int x){goto again;again:x='broken
work(x);x++;return x;}
int valid_escaped_string(int x){char *s="broken\";";work(x);x++;return x;}
int valid_escaped_char(int x){char c='\'';work(x);x++;return x;}
int after_literal_extension(int x){work(x);return x;}
