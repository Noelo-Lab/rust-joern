int try_null(int x) { try { ; } catch (...) { x++; } return x; }
int try_local(int x) { try { int y; } catch (...) { x++; } return x; }
int try_local_return(int x) { try { int y; } catch (...) { return x+1; } return x; }
