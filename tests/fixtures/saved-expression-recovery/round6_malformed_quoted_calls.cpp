int known();
int format_char_words_statement(int x,int rsi,int rbx,int rbp){
known(1, rsi, '%s' is not a recognizable number.");
x++;return x;
}
int format_char_words_scalar_if(int x,int rsi,int rbx,int rbp){
if(x)known(1, rsi, '%s' is not a recognizable number.");
x++;return x;
}
int format_char_words_braced_if(int x,int rsi,int rbx,int rbp){
if(x){
known(1, rsi, '%s' is not a recognizable number.");
}
x++;return x;
}
int format_char_words_scalar_while(int x,int rsi,int rbx,int rbp){
while(x)known(1, rsi, '%s' is not a recognizable number.");
x++;return x;
}
int format_char_words_braced_while(int x,int rsi,int rbx,int rbp){
while(x){
known(1, rsi, '%s' is not a recognizable number.");
}
x++;return x;
}
int format_char_words_return(int x,int rsi,int rbx,int rbp){
return known(1, rsi, '%s' is not a recognizable number.");

}
int format_char_words_initializer(int x,int rsi,int rbx,int rbp){
int y=known(1, rsi, '%s' is not a recognizable number.");
return x;
}
int format_char_words_condition(int x,int rsi,int rbx,int rbp){
if(known(1, rsi, '%s' is not a recognizable number."))x++;
return x;
}
int escaped_assert_args_statement(int x,int rsi,int rbx,int rbp){
known("!\"unexpected error code from argv_iter\", "src/wc.c", 0x3aa, "main");
x++;return x;
}
int escaped_assert_args_scalar_if(int x,int rsi,int rbx,int rbp){
if(x)known("!\"unexpected error code from argv_iter\", "src/wc.c", 0x3aa, "main");
x++;return x;
}
int escaped_assert_args_braced_if(int x,int rsi,int rbx,int rbp){
if(x){
known("!\"unexpected error code from argv_iter\", "src/wc.c", 0x3aa, "main");
}
x++;return x;
}
int escaped_assert_args_scalar_while(int x,int rsi,int rbx,int rbp){
while(x)known("!\"unexpected error code from argv_iter\", "src/wc.c", 0x3aa, "main");
x++;return x;
}
int escaped_assert_args_braced_while(int x,int rsi,int rbx,int rbp){
while(x){
known("!\"unexpected error code from argv_iter\", "src/wc.c", 0x3aa, "main");
}
x++;return x;
}
int escaped_assert_args_return(int x,int rsi,int rbx,int rbp){
return known("!\"unexpected error code from argv_iter\", "src/wc.c", 0x3aa, "main");

}
int escaped_assert_args_initializer(int x,int rsi,int rbx,int rbp){
int y=known("!\"unexpected error code from argv_iter\", "src/wc.c", 0x3aa, "main");
return x;
}
int escaped_assert_args_condition(int x,int rsi,int rbx,int rbp){
if(known("!\"unexpected error code from argv_iter\", "src/wc.c", 0x3aa, "main"))x++;
return x;
}
int unclosed_format_string_statement(int x,int rsi,int rbx,int rbp){
known("missing operand after '%s', *((rbx + rbp*8 - 8)));
x++;return x;
}
int unclosed_format_string_scalar_if(int x,int rsi,int rbx,int rbp){
if(x)known("missing operand after '%s', *((rbx + rbp*8 - 8)));
x++;return x;
}
int unclosed_format_string_braced_if(int x,int rsi,int rbx,int rbp){
if(x){
known("missing operand after '%s', *((rbx + rbp*8 - 8)));
}
x++;return x;
}
int unclosed_format_string_scalar_while(int x,int rsi,int rbx,int rbp){
while(x)known("missing operand after '%s', *((rbx + rbp*8 - 8)));
x++;return x;
}
int unclosed_format_string_braced_while(int x,int rsi,int rbx,int rbp){
while(x){
known("missing operand after '%s', *((rbx + rbp*8 - 8)));
}
x++;return x;
}
int unclosed_format_string_return(int x,int rsi,int rbx,int rbp){
return known("missing operand after '%s', *((rbx + rbp*8 - 8)));

}
int unclosed_format_string_initializer(int x,int rsi,int rbx,int rbp){
int y=known("missing operand after '%s', *((rbx + rbp*8 - 8)));
return x;
}
int unclosed_format_string_condition(int x,int rsi,int rbx,int rbp){
if(known("missing operand after '%s', *((rbx + rbp*8 - 8))))x++;
return x;
}
int char_words_statement(int x,int rsi,int rbx,int rbp){
known('-' specified for more than one input file");
x++;return x;
}
int char_words_scalar_if(int x,int rsi,int rbx,int rbp){
if(x)known('-' specified for more than one input file");
x++;return x;
}
int char_words_braced_if(int x,int rsi,int rbx,int rbp){
if(x){
known('-' specified for more than one input file");
}
x++;return x;
}
int char_words_scalar_while(int x,int rsi,int rbx,int rbp){
while(x)known('-' specified for more than one input file");
x++;return x;
}
int char_words_braced_while(int x,int rsi,int rbx,int rbp){
while(x){
known('-' specified for more than one input file");
}
x++;return x;
}
int char_words_return(int x,int rsi,int rbx,int rbp){
return known('-' specified for more than one input file");

}
int char_words_initializer(int x,int rsi,int rbx,int rbp){
int y=known('-' specified for more than one input file");
return x;
}
int char_words_condition(int x,int rsi,int rbx,int rbp){
if(known('-' specified for more than one input file"))x++;
return x;
}
int plain_unclosed_string_statement(int x,int rsi,int rbx,int rbp){
known("unclosed);
x++;return x;
}
int plain_unclosed_string_scalar_if(int x,int rsi,int rbx,int rbp){
if(x)known("unclosed);
x++;return x;
}
int plain_unclosed_string_braced_if(int x,int rsi,int rbx,int rbp){
if(x){
known("unclosed);
}
x++;return x;
}
int plain_unclosed_string_scalar_while(int x,int rsi,int rbx,int rbp){
while(x)known("unclosed);
x++;return x;
}
int plain_unclosed_string_braced_while(int x,int rsi,int rbx,int rbp){
while(x){
known("unclosed);
}
x++;return x;
}
int plain_unclosed_string_return(int x,int rsi,int rbx,int rbp){
return known("unclosed);

}
int plain_unclosed_string_initializer(int x,int rsi,int rbx,int rbp){
int y=known("unclosed);
return x;
}
int plain_unclosed_string_condition(int x,int rsi,int rbx,int rbp){
if(known("unclosed))x++;
return x;
}
int braced_if_following_call(int x,int rsi,int rbx,int rbp){
if(x){known("unclosed);
}
known(x);return x;
}
int scalar_if_following_call(int x,int rsi,int rbx,int rbp){
if(x)known("unclosed);
known(x);return x;
}
int return_following_call(int x,int rsi,int rbx,int rbp){
return known("unclosed);
known(x);return x;
}
int initializer_following_call(int x,int rsi,int rbx,int rbp){
int y=known("unclosed);
known(x);return x;
}
int positive_words(int x,int rsi,int rbx,int rbp){
known("%s is a recognizable number.");
x++;return x;
}
int positive_escaped_quotes(int x,int rsi,int rbx,int rbp){
known("!\"message\"", "file.c",1,"fn");
x++;return x;
}
int positive_format_argument(int x,int rsi,int rbx,int rbp){
known("missing operand after %s",x);
x++;return x;
}
int positive_char_argument(int x,int rsi,int rbx,int rbp){
known('-');
x++;return x;
}
int positive_string_if(int x,int rsi,int rbx,int rbp){
if(x){known("closed");}
return x;
}
int positive_string_return(int x,int rsi,int rbx,int rbp){
return known("closed");
}
int positive_string_initializer(int x,int rsi,int rbx,int rbp){
int y=known("closed");
return y;
}
int positive_string_condition(int x,int rsi,int rbx,int rbp){
if(known("closed"))x++;
return x;
}
int after_control(int x,int rsi,int rbx,int rbp){
return x;
}
