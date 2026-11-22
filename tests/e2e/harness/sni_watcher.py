"""A stand-in for the panel's StatusNotifierWatcher (E8-06): owns
org.kde.StatusNotifierWatcher on the test session bus and writes each item
that registers to the file given as the only argument, one per line."""

import sys

import gi

gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib  # noqa: E402

XML = """
<node>
  <interface name="org.kde.StatusNotifierWatcher">
    <method name="RegisterStatusNotifierItem"><arg type="s" name="service" direction="in"/></method>
    <method name="RegisterStatusNotifierHost"><arg type="s" name="service" direction="in"/></method>
    <property name="RegisteredStatusNotifierItems" type="as" access="read"/>
    <property name="IsStatusNotifierHostRegistered" type="b" access="read"/>
    <property name="ProtocolVersion" type="i" access="read"/>
    <signal name="StatusNotifierItemRegistered"><arg type="s" name="service"/></signal>
  </interface>
</node>"""

LOG = sys.argv[1]
items: list[str] = []


def on_call(connection, sender, path, interface, method, params, invocation):
    if method == "RegisterStatusNotifierItem":
        service = params.unpack()[0]
        # A bare object path means "this sender, at that path".
        item = service if not service.startswith("/") else f"{sender}{service}"
        items.append(item)
        with open(LOG, "a") as log:
            log.write(item + "\n")
    invocation.return_value(None)


def on_get(connection, sender, path, interface, name):
    return {
        "RegisteredStatusNotifierItems": GLib.Variant("as", items),
        "IsStatusNotifierHostRegistered": GLib.Variant("b", True),
        "ProtocolVersion": GLib.Variant("i", 0),
    }[name]


def on_bus(connection, name):
    info = Gio.DBusNodeInfo.new_for_xml(XML).interfaces[0]
    connection.register_object("/StatusNotifierWatcher", info, on_call, on_get, None)


def on_name(connection, name):
    with open(LOG, "a") as log:
        log.write("READY\n")


Gio.bus_own_name(Gio.BusType.SESSION, "org.kde.StatusNotifierWatcher", Gio.BusNameOwnerFlags.NONE,
                 on_bus, on_name, lambda *a: sys.exit(1))
GLib.MainLoop().run()
