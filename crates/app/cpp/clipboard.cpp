#include "steward/clipboard.h"

#include <QtCore/QMimeData>
#include <QtGui/QClipboard>
#include <QtGui/QGuiApplication>

void clipboard_set(const rust::Vec<rust::String>& mimes, const rust::Vec<rust::String>& data)
{
    auto* mime = new QMimeData();
    for (size_t i = 0; i < mimes.size() && i < data.size(); ++i) {
        mime->setData(QString::fromUtf8(mimes[i].data(), static_cast<qsizetype>(mimes[i].size())),
                      QByteArray(data[i].data(), static_cast<qsizetype>(data[i].size())));
    }
    // The clipboard takes ownership of `mime`.
    QGuiApplication::clipboard()->setMimeData(mime);
}

rust::String clipboard_get(rust::Str mime)
{
    const QMimeData* offered = QGuiApplication::clipboard()->mimeData();
    if (!offered) {
        return rust::String();
    }
    const QByteArray bytes = offered->data(QString::fromUtf8(mime.data(), static_cast<qsizetype>(mime.size())));
    return rust::String::lossy(bytes.constData(), static_cast<size_t>(bytes.size()));
}
