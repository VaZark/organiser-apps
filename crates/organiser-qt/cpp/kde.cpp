// The KDE parts that have no Rust binding: the KI18n context for QML (`i18n(...)`).
#include <KLocalizedQmlContext>
#include <KLocalizedString>
#include <QQmlApplicationEngine>
#include <QQmlContext>

extern "C" void organiser_qt_setup_kde(void *engine, const char *domain) {
    auto *eng = static_cast<QQmlApplicationEngine *>(engine);
    KLocalizedString::setApplicationDomain(QByteArray(domain));
    auto *ctx = new KLocalizedQmlContext(eng);
    eng->rootContext()->setContextObject(ctx);
    QQmlEngine::setContextForObject(ctx, eng->rootContext());
    ctx->setTranslationDomain(QString::fromUtf8(domain));
}
