namespace demo {
void release();
struct Record { Record(); ~Record(); };
Record::Record() { release(); }
Record::~Record() { release(); }
}
