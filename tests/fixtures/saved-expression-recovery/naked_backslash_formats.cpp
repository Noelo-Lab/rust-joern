int work(int);
int print(const char *, ...);
int naked_format_statement(int x){char *format;format="";\n\n";
work(x);x++;return x;}
int naked_format_in_block(int x){char *format;if(x){format="";\n\n";
work(x);}x++;return x;}
int naked_format_label(int x){char *format;again:format="";\n\n";
work(x);x++;return x;}
int naked_backslash_before_argument(int x){x=work(\x);work(x);return x;}
int doubled_naked_backslash(int x){x=work(\\x);work(x);return x;}
int naked_backslash_in_identifier(int x){int name=x;return na\me;work(x);return x;}
int naked_backslash_in_initializer(int x){int name=work(\x);work(name);return x;}
int naked_n_letters(int x){n\n;work(x);x++;return x;}
int valid_quoted_backslashes(int x){print("\\n\\n",x);work(x);return x;}
int valid_quoted_backslash_char(int x){char c='\\';work(x);return x;}
int valid_ucn_initial(int x){int \u00e9=x;work(\u00e9);return x;}
int valid_ucn_inside(int x){int a\u00e9=x;work(a\u00e9);return x;}
int valid_ucn_upper(int x){int \U000000e9=x;work(\U000000e9);return x;}
int valid_splice_operator(int x){x=x+\
1;work(x);return x;}
int valid_splice_string(int x){print("a\
b",x);work(x);return x;}
int valid_splice_identifier(int x){int ab=x;work(a\
b);return x;}
int after_lexical_formats(int x){work(x);return x;}
