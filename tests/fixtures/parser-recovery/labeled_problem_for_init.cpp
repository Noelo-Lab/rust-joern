int bad_scalar(int j){goto label;label:for(j=::stream;j;j=j+1)foo(j);return j;}
int bad_braced(int j){goto label;label:for(j=::stream;j;j=j+1){foo(j);}return j;}
int bad_null(int j){goto label;label:for(j=::stream;j;j=j+1);return j;}
int bad_local(int j){goto label;label:for(j=::stream;j;j=j+1)int x=j;return j;}
int good_scalar(int j){goto label;label:for(j=stream;j;j=j+1)foo(j);return j;}
int good_braced(int j){goto label;label:for(j=stream;j;j=j+1){foo(j);}return j;}
