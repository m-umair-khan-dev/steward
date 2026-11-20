// Loaded only when Qt has QtQuick.Effects (Qt 6.5+); see FxBlur.qml.
import QtQuick
import QtQuick.Effects

MultiEffect {
    blurEnabled: true
    blur: 1.0
    blurMax: 40
    maskEnabled: true
    maskThresholdMin: 0.5
    maskSpreadAtMin: 1.0
}
