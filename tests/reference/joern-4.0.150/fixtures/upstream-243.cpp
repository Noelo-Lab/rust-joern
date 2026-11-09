
class A {
  public:
    static int square(int num) {
      return num * num;
    }
};

void call_square() {
  A::square(10);
}
