import sys

def main():
    a = sys.argv[1:]
    cfg = {
        "tb_cmd": a[0] if len(a) > 0 else "thunderbird",
        "auto_start": a[1] if len(a) > 1 else "true",
        "badge_color": a[2] if len(a) > 2 else "#FF0000",
        "badge_text_color": a[3] if len(a) > 3 else "#FFFFFF",
        "poll_interval": a[4] if len(a) > 4 else "5",
    }

    result = try_qt(cfg)
    if result is None:
        result = try_tkinter(cfg)
    if result is None:
        sys.exit(2)
    sys.exit(0 if result else 1)


def try_qt(cfg):
    try:
        from PyQt6.QtWidgets import (
            QApplication, QDialog, QVBoxLayout, QHBoxLayout, QFormLayout,
            QGroupBox, QLineEdit, QCheckBox, QSpinBox, QPushButton, QColorDialog,
        )
        from PyQt6.QtGui import QColor
    except ImportError:
        try:
            from PySide6.QtWidgets import (
                QApplication, QDialog, QVBoxLayout, QHBoxLayout, QFormLayout,
                QGroupBox, QLineEdit, QCheckBox, QSpinBox, QPushButton, QColorDialog,
            )
            from PySide6.QtGui import QColor
        except ImportError:
            try:
                from PyQt5.QtWidgets import (
                    QApplication, QDialog, QVBoxLayout, QHBoxLayout, QFormLayout,
                    QGroupBox, QLineEdit, QCheckBox, QSpinBox, QPushButton, QColorDialog,
                )
                from PyQt5.QtGui import QColor
            except ImportError:
                return None

    app = QApplication(sys.argv[:1])
    dlg = QDialog()
    dlg.setWindowTitle("ThunderTray Settings")
    dlg.setMinimumWidth(420)

    layout = QVBoxLayout(dlg)
    layout.setSpacing(12)

    # --- General ---
    gen_group = QGroupBox("General")
    gen_form = QFormLayout()
    gen_group.setLayout(gen_form)

    cmd_edit = QLineEdit(cfg["tb_cmd"])
    cmd_edit.setPlaceholderText("e.g. thunderbird or /usr/bin/thunderbird")
    gen_form.addRow("Thunderbird command:", cmd_edit)

    auto_check = QCheckBox("Auto-start Thunderbird with ThunderTray")
    auto_check.setChecked(cfg["auto_start"].lower() == "true")
    gen_form.addRow(auto_check)

    layout.addWidget(gen_group)

    # --- Appearance ---
    app_group = QGroupBox("Appearance")
    app_form = QFormLayout()
    app_group.setLayout(app_form)

    def make_color_btn(initial, title):
        btn = QPushButton(initial)
        btn.setFixedHeight(28)
        btn.setMinimumWidth(110)
        btn._color = initial

        def update_style():
            c = QColor(btn._color)
            lum = 0.299 * c.red() + 0.587 * c.green() + 0.114 * c.blue()
            tc = "#000" if lum > 128 else "#fff"
            btn.setStyleSheet(
                f"background-color: {btn._color}; color: {tc}; "
                f"border: 1px solid palette(mid); padding: 2px 8px; font-family: monospace;"
            )
            btn.setText(btn._color)

        update_style()

        def pick():
            c = QColorDialog.getColor(QColor(btn._color), dlg, title)
            if c.isValid():
                btn._color = c.name().upper()
                update_style()

        btn.clicked.connect(pick)
        return btn

    badge_btn = make_color_btn(cfg["badge_color"], "Badge Color")
    app_form.addRow("Badge color:", badge_btn)

    text_btn = make_color_btn(cfg["badge_text_color"], "Badge Text Color")
    app_form.addRow("Badge text color:", text_btn)

    layout.addWidget(app_group)

    # --- Monitoring ---
    mon_group = QGroupBox("Monitoring")
    mon_form = QFormLayout()
    mon_group.setLayout(mon_form)

    poll_spin = QSpinBox()
    poll_spin.setRange(1, 3600)
    poll_spin.setValue(int(cfg["poll_interval"]))
    poll_spin.setSuffix(" seconds")
    mon_form.addRow("Check interval:", poll_spin)

    layout.addWidget(mon_group)

    # --- Buttons ---
    layout.addStretch()
    btn_row = QHBoxLayout()
    btn_row.addStretch()

    cancel_btn = QPushButton("Cancel")
    cancel_btn.clicked.connect(dlg.reject)
    btn_row.addWidget(cancel_btn)

    save_btn = QPushButton("Save")
    save_btn.setDefault(True)
    save_btn.clicked.connect(dlg.accept)
    btn_row.addWidget(save_btn)

    layout.addLayout(btn_row)

    if dlg.exec() == 1:  # QDialog.Accepted
        print(cmd_edit.text())
        print("true" if auto_check.isChecked() else "false")
        print(badge_btn._color)
        print(text_btn._color)
        print(poll_spin.value())
        return True
    return False


def try_tkinter(cfg):
    try:
        import tkinter as tk
        from tkinter import ttk, colorchooser
    except ImportError:
        return None

    result = {"saved": False}

    root = tk.Tk()
    root.title("ThunderTray Settings")
    root.resizable(False, False)

    frame = ttk.Frame(root, padding=15)
    frame.pack(fill=tk.BOTH, expand=True)

    # --- General ---
    ttk.Label(frame, text="General", font=("", 10, "bold")).pack(anchor=tk.W, pady=(0, 4))

    cmd_frame = ttk.Frame(frame)
    cmd_frame.pack(fill=tk.X, pady=2)
    ttk.Label(cmd_frame, text="Thunderbird command:").pack(side=tk.LEFT)
    cmd_var = tk.StringVar(value=cfg["tb_cmd"])
    ttk.Entry(cmd_frame, textvariable=cmd_var, width=25).pack(side=tk.RIGHT, fill=tk.X, expand=True, padx=(8, 0))

    auto_var = tk.BooleanVar(value=cfg["auto_start"].lower() == "true")
    ttk.Checkbutton(frame, text="Auto-start Thunderbird with ThunderTray", variable=auto_var).pack(anchor=tk.W, pady=2)

    ttk.Separator(frame, orient=tk.HORIZONTAL).pack(fill=tk.X, pady=8)

    # --- Appearance ---
    ttk.Label(frame, text="Appearance", font=("", 10, "bold")).pack(anchor=tk.W, pady=(0, 4))

    colors = {"badge": cfg["badge_color"], "text": cfg["badge_text_color"]}

    def make_color_row(parent, label, key):
        row = ttk.Frame(parent)
        row.pack(fill=tk.X, pady=2)
        ttk.Label(row, text=label).pack(side=tk.LEFT)
        swatch = tk.Label(
            row, text=colors[key], width=10, bg=colors[key],
            relief=tk.SUNKEN, bd=1, font=("monospace", 9),
        )
        try:
            c = root.winfo_rgb(colors[key])
            lum = (0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]) / 256
            swatch.configure(fg="#000" if lum > 128 else "#fff")
        except Exception:
            pass
        swatch.pack(side=tk.RIGHT, padx=(8, 0))

        def pick():
            c = colorchooser.askcolor(color=colors[key], title=label, parent=root)
            if c[1]:
                colors[key] = c[1].upper()
                swatch.configure(bg=colors[key], text=colors[key])
                try:
                    rgb = root.winfo_rgb(colors[key])
                    lum = (0.299 * rgb[0] + 0.587 * rgb[1] + 0.114 * rgb[2]) / 256
                    swatch.configure(fg="#000" if lum > 128 else "#fff")
                except Exception:
                    pass

        ttk.Button(row, text="Pick\u2026", command=pick).pack(side=tk.RIGHT, padx=4)
        swatch.bind("<Button-1>", lambda e: pick())

    make_color_row(frame, "Badge color:", "badge")
    make_color_row(frame, "Badge text color:", "text")

    ttk.Separator(frame, orient=tk.HORIZONTAL).pack(fill=tk.X, pady=8)

    # --- Monitoring ---
    ttk.Label(frame, text="Monitoring", font=("", 10, "bold")).pack(anchor=tk.W, pady=(0, 4))

    poll_frame = ttk.Frame(frame)
    poll_frame.pack(fill=tk.X, pady=2)
    ttk.Label(poll_frame, text="Check interval:").pack(side=tk.LEFT)
    ttk.Label(poll_frame, text="seconds").pack(side=tk.RIGHT)
    poll_var = tk.StringVar(value=cfg["poll_interval"])
    ttk.Spinbox(poll_frame, from_=1, to=3600, textvariable=poll_var, width=6).pack(side=tk.RIGHT, padx=4)

    # --- Buttons ---
    ttk.Separator(frame, orient=tk.HORIZONTAL).pack(fill=tk.X, pady=8)
    btn_frame = ttk.Frame(frame)
    btn_frame.pack(fill=tk.X)

    def on_save():
        print(cmd_var.get())
        print("true" if auto_var.get() else "false")
        print(colors["badge"])
        print(colors["text"])
        try:
            v = int(poll_var.get())
        except ValueError:
            v = int(cfg["poll_interval"])
        print(v)
        result["saved"] = True
        root.destroy()

    ttk.Button(btn_frame, text="Save", command=on_save).pack(side=tk.RIGHT, padx=(4, 0))
    ttk.Button(btn_frame, text="Cancel", command=root.destroy).pack(side=tk.RIGHT)

    root.mainloop()
    return result["saved"]


main()
