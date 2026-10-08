namespace std {
class type_info {
  const char *__type_name;
public:
  virtual ~type_info();
  type_info();
  type_info(const type_info &rhs);
};
}
using std::type_info;
type_info::~type_info() {}
type_info::type_info(const type_info &rhs) {
  __type_name = rhs.__type_name;
}

class Plain {
public:
  Plain();
  Plain(const Plain &rhs);
  ~Plain();
};
