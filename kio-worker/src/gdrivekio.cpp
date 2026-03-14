// KIO worker stub — Phase 5: listDir, stat, get, put, del, mkdir, etc. via D-Bus.

#include <KIO/WorkerBase>

class GDriveWorker : public KIO::WorkerBase
{
public:
    GDriveWorker(const QByteArray &pool, const QByteArray &app)
        : KIO::WorkerBase(pool, app) {}

    void listDir(const QUrl &url) override;
    void stat(const QUrl &url) override;
    void get(const QUrl &url) override;
    // put, del, mkdir, copy, rename — Phase 5
};

void GDriveWorker::listDir(const QUrl &) { error(KIO::ERR_NOT_IMPLEMENTED, QString()); }
void GDriveWorker::stat(const QUrl &) { error(KIO::ERR_NOT_IMPLEMENTED, QString()); }
void GDriveWorker::get(const QUrl &) { error(KIO::ERR_NOT_IMPLEMENTED, QString()); }

KIO_WORKER_MAIN(GDriveWorker)
