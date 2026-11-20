int tick(int);
struct Window {
  int width;
  int height;
  Window(int w, int h) : width(tick(w)), height(h) {
    if (width > height) width = height;
  }
  Window() : width(tick(0)), height(0) {}
};
