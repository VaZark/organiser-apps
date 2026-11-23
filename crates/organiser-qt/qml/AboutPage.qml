import QtQuick
import org.kde.kirigami as Kirigami

/// The about page, from the app's metadata.
Kirigami.AboutPage {
    required property string appName
    required property string appComment
    property string appId: ""
    aboutData: ({
        componentName: appName.toLowerCase(),
        productName: "organiser/" + appName.toLowerCase(),
        displayName: appName,
        shortDescription: appComment,
        version: "0.0.3",
        homepage: "https://github.com/VaZark",
        bugAddress: "",
        desktopFileName: appId,
        programLogo: appId,
        licenses: [{ name: "MIT License", text: "", spdx: "MIT" }],
        authors: [],
        credits: [],
        translators: [],
        copyrightStatement: "© Vasanth M"
    })
}
