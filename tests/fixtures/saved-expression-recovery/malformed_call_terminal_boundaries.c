int known(const char *);
int work(int);
int baseline(int x){known("closed");work(x);return x;}
int call_terminal(int x){
    known("broken);
}
int call_followed_call(int x){
    known("broken);
    work(x);return x;
}
int call_followed_assignment(int x){
    known("broken);
    x=work(x);return x;
}
int call_inner_brace(int x){if(x){
    known("broken);
}work(x);return x;}
int call_scalar_if(int x){if(x)
    known("broken);
    work(x);return x;
}
int call_scalar_while(int x){while(x)
    known("broken);
    work(x);return x;
}
int call_braced_while(int x){while(x){
    known("broken);
}work(x);return x;}
int call_scalar_do(int x){do
    known("broken);
while(x);work(x);return x;}
int call_braced_do(int x){do{
    known("broken);
}while(x);work(x);return x;}
int return_terminal(int x){
    return known("broken);
}
int return_followed_call(int x){
    return known("broken);
    work(x);return x;
}
int return_inner_brace(int x){if(x){
    return known("broken);
}work(x);return x;}
int initializer_terminal(int x){
    int y=known("broken);
}
int initializer_followed_call(int x){
    int y=known("broken);
    work(x);return x;
}
int initializer_inner_brace(int x){if(x){
    int y=known("broken);
}work(x);return x;}
int assignment_terminal(int x){
    x=known("broken);
}
int assignment_inner_brace(int x){if(x){
    x=known("broken);
}work(x);return x;}
int condition_inner_brace(int x){if(known("broken)){
    work(x);
}work(x);return x;}
int valid_braced_call(int x){if(x){known("closed");}work(x);return x;}
int valid_scalar_call(int x){if(x)known("closed");work(x);return x;}
int newline_literal_with_real_terminator(int x){
    known("broken
    );work(x);return x;
}
int newline_literal_separate_semicolon(int x){
    known("broken
    ;work(x);return x;
}
int escaped_literal(int x){known("first\
second");work(x);return x;}
int after_control(int x){return x;}
