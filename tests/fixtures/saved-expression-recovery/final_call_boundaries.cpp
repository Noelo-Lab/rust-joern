int known(int);
int opaque(int,int);
int multiline_quote_before_brace(int x){
sub_479918("ssh.c", "main", 0x357, 0, 1, 0, 0, 
"Only a single -J option is permitted (use commas to separate multiple jump 
hops)");
}
int multiline_quote_after_brace(int x){known(x);return x;
}
int multiline_quote_before_following_call(int x){
sub_479918("ssh.c", "main", 0x357, 0, 1, 0, 0, 
"Only a single -J option is permitted (use commas to separate multiple jump 
hops)");
known(x);return x;
}
int multiline_quote_scalar_if(int x){
if(x)
sub_479918("ssh.c", "main", 0x357, 0, 1, 0, 0, 
"Only a single -J option is permitted (use commas to separate multiple jump 
hops)");
known(x);return x;
}
int multiline_quote_braced_if(int x){
if(x){
sub_479918("ssh.c", "main", 0x357, 0, 1, 0, 0, 
"Only a single -J option is permitted (use commas to separate multiple jump 
hops)");
}
known(x);return x;
}
int multiline_quote_scalar_while(int x){
while(x)
sub_479918("ssh.c", "main", 0x357, 0, 1, 0, 0, 
"Only a single -J option is permitted (use commas to separate multiple jump 
hops)");
known(x);return x;
}
int multiline_quote_braced_while(int x){
while(x){
sub_479918("ssh.c", "main", 0x357, 0, 1, 0, 0, 
"Only a single -J option is permitted (use commas to separate multiple jump 
hops)");
}
known(x);return x;
}
int multiline_quote_return(int x){
return sub_479918("ssh.c", "main", 0x357, 0, 1, 0, 0, 
"Only a single -J option is permitted (use commas to separate multiple jump 
hops)");
known(x);return x;
}
int multiline_quote_initializer(int x){
int y=sub_479918("ssh.c", "main", 0x357, 0, 1, 0, 0, 
"Only a single -J option is permitted (use commas to separate multiple jump 
hops)");
known(x);return x;
}
int opaque_call_before_brace(int x){
amd64g_dirtyhelper_storeF80le((Reference vvar_2329{s-248|8b}), Reinterpret(F64->I64, (((unsupported_<class 'pyvex.expr.GetI'>() CmpNE 0<8>)) ? (unsupported_<class 'pyvex.expr.GetI'>()) : (nan<64>))))
}
int opaque_call_after_brace(int x){known(x);return x;
}
int opaque_call_before_following_call(int x){
amd64g_dirtyhelper_storeF80le((Reference vvar_2329{s-248|8b}), Reinterpret(F64->I64, (((unsupported_<class 'pyvex.expr.GetI'>() CmpNE 0<8>)) ? (unsupported_<class 'pyvex.expr.GetI'>()) : (nan<64>))))
known(x);return x;
}
int opaque_call_scalar_if(int x){
if(x)
amd64g_dirtyhelper_storeF80le((Reference vvar_2329{s-248|8b}), Reinterpret(F64->I64, (((unsupported_<class 'pyvex.expr.GetI'>() CmpNE 0<8>)) ? (unsupported_<class 'pyvex.expr.GetI'>()) : (nan<64>))))
known(x);return x;
}
int opaque_call_braced_if(int x){
if(x){
amd64g_dirtyhelper_storeF80le((Reference vvar_2329{s-248|8b}), Reinterpret(F64->I64, (((unsupported_<class 'pyvex.expr.GetI'>() CmpNE 0<8>)) ? (unsupported_<class 'pyvex.expr.GetI'>()) : (nan<64>))))
}
known(x);return x;
}
int opaque_call_scalar_while(int x){
while(x)
amd64g_dirtyhelper_storeF80le((Reference vvar_2329{s-248|8b}), Reinterpret(F64->I64, (((unsupported_<class 'pyvex.expr.GetI'>() CmpNE 0<8>)) ? (unsupported_<class 'pyvex.expr.GetI'>()) : (nan<64>))))
known(x);return x;
}
int opaque_call_braced_while(int x){
while(x){
amd64g_dirtyhelper_storeF80le((Reference vvar_2329{s-248|8b}), Reinterpret(F64->I64, (((unsupported_<class 'pyvex.expr.GetI'>() CmpNE 0<8>)) ? (unsupported_<class 'pyvex.expr.GetI'>()) : (nan<64>))))
}
known(x);return x;
}
int opaque_call_return(int x){
return amd64g_dirtyhelper_storeF80le((Reference vvar_2329{s-248|8b}), Reinterpret(F64->I64, (((unsupported_<class 'pyvex.expr.GetI'>() CmpNE 0<8>)) ? (unsupported_<class 'pyvex.expr.GetI'>()) : (nan<64>))))
known(x);return x;
}
int opaque_call_initializer(int x){
int y=amd64g_dirtyhelper_storeF80le((Reference vvar_2329{s-248|8b}), Reinterpret(F64->I64, (((unsupported_<class 'pyvex.expr.GetI'>() CmpNE 0<8>)) ? (unsupported_<class 'pyvex.expr.GetI'>()) : (nan<64>))))
known(x);return x;
}
int opaque_two_before_brace(int x){
amd64g_dirtyhelper_storeF80le((Reference vvar_2329{s-248|8b}), Reinterpret(F64->I64, (((unsupported_<class 'pyvex.expr.GetI'>() CmpNE 0<8>)) ? (unsupported_<class 'pyvex.expr.GetI'>()) : (nan<64>))))
amd64g_dirtyhelper_storeF80le((Reference vvar_2394{s-232|8b}), Reinterpret(F64->I64, (((unsupported_<class 'pyvex.expr.GetI'>() CmpNE 0<8>)) ? (unsupported_<class 'pyvex.expr.GetI'>()) : (nan<64>))))
}
int opaque_two_after_brace(int x){known(x);return x;
}
int opaque_two_braced_if(int x){
if(x){
amd64g_dirtyhelper_storeF80le((Reference vvar_2329{s-248|8b}), Reinterpret(F64->I64, (((unsupported_<class 'pyvex.expr.GetI'>() CmpNE 0<8>)) ? (unsupported_<class 'pyvex.expr.GetI'>()) : (nan<64>))))
amd64g_dirtyhelper_storeF80le((Reference vvar_2394{s-232|8b}), Reinterpret(F64->I64, (((unsupported_<class 'pyvex.expr.GetI'>() CmpNE 0<8>)) ? (unsupported_<class 'pyvex.expr.GetI'>()) : (nan<64>))))
}
known(x);return x;
}
int opaque_two_braced_while(int x){
while(x){
amd64g_dirtyhelper_storeF80le((Reference vvar_2329{s-248|8b}), Reinterpret(F64->I64, (((unsupported_<class 'pyvex.expr.GetI'>() CmpNE 0<8>)) ? (unsupported_<class 'pyvex.expr.GetI'>()) : (nan<64>))))
amd64g_dirtyhelper_storeF80le((Reference vvar_2394{s-232|8b}), Reinterpret(F64->I64, (((unsupported_<class 'pyvex.expr.GetI'>() CmpNE 0<8>)) ? (unsupported_<class 'pyvex.expr.GetI'>()) : (nan<64>))))
}
known(x);return x;
}
int opaque_proper_semicolon(int x){
amd64g_dirtyhelper_storeF80le((Reference vvar_2329{s-248|8b}), Reinterpret(F64->I64, (((unsupported_<class 'pyvex.expr.GetI'>() CmpNE 0<8>)) ? (unsupported_<class 'pyvex.expr.GetI'>()) : (nan<64>))));
known(x);return x;
}
int opaque_two_proper_semicolons(int x){
amd64g_dirtyhelper_storeF80le((Reference vvar_2329{s-248|8b}), Reinterpret(F64->I64, (((unsupported_<class 'pyvex.expr.GetI'>() CmpNE 0<8>)) ? (unsupported_<class 'pyvex.expr.GetI'>()) : (nan<64>))));
amd64g_dirtyhelper_storeF80le((Reference vvar_2394{s-232|8b}), Reinterpret(F64->I64, (((unsupported_<class 'pyvex.expr.GetI'>() CmpNE 0<8>)) ? (unsupported_<class 'pyvex.expr.GetI'>()) : (nan<64>))));
known(x);return x;
}
int valid_quoted_splice(int x){
sub_479918("ssh.c", "main", 0x357, 0, 1, 0, 0, 
"Only a single -J option is permitted (use commas to separate multiple jump \
hops)");
known(x);return x;
}
int valid_adjacent_strings(int x){
sub_479918("ssh.c", "main", 0x357, 0, 1, 0, 0, 
"Only a single -J option is permitted (use commas to separate multiple jump "
"hops)");
known(x);return x;
}
int valid_nested_call(int x){
opaque((x),known(x));known(x);return x;
}
int valid_call_separator(int x){
known(x)
known(x);return x;
}
int valid_nested_if(int x){
if(x){opaque((x),known(x));}known(x);return x;
}
int after_control(int x){
known(x);return x;
}
