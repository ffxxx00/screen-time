using System;
using System.Diagnostics;
using System.Drawing;
using System.IO;
using System.IO.Compression;
using System.Reflection;
using System.Security.Cryptography;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using System.Windows.Forms;

// The EXE embeds the same ZIP used by the transactional PowerShell installer.
// It installs for the current user and never requests elevation.
internal static class Setup
{
    [STAThread]
    private static int Main(string[] args)
    {
        bool created;
        using (Mutex mutex = new Mutex(true, "Local\\ScreenTime.Setup", out created))
        {
            if (!created)
            {
                if (args.Length == 0) MessageBox.Show("Ya hay una instalación de Screen Time en curso.", "Screen Time", MessageBoxButtons.OK, MessageBoxIcon.Information);
                return 3;
            }
            try { return Run(args); }
            finally { mutex.ReleaseMutex(); }
        }
    }

    private static int Run(string[] args)
    {
        if (args.Length == 1 && (args[0] == "--verify" || args[0] == "--silent"))
        {
            try { Deploy(args[0] == "--verify"); return 0; }
            catch (Exception error) { Console.Error.WriteLine(error.Message); return 1; }
        }
        if (args.Length != 0) return 2;
        Application.EnableVisualStyles();
        Application.SetCompatibleTextRenderingDefault(false);
        Application.Run(new SetupWindow());
        return 0;
    }

    internal static string InstalledExecutable
    {
        get { return Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Programs", "Screen Time", "screen-time.exe"); }
    }

    private static string Quote(string value)
    {
        if (value.IndexOf('"') >= 0) throw new InvalidOperationException("Ruta no válida.");
        return "\"" + value + "\"";
    }

    internal static void Deploy(bool verifyOnly)
    {
        string temp = Path.GetFullPath(Path.GetTempPath()).TrimEnd(Path.DirectorySeparatorChar);
        string root = Path.Combine(temp, "screen-time-setup-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(root);
        try
        {
            Assembly assembly = Assembly.GetExecutingAssembly();
            byte[] payload;
            using (Stream resource = assembly.GetManifestResourceStream("ScreenTime.Payload"))
            using (MemoryStream memory = new MemoryStream())
            {
                if (resource == null) throw new InvalidOperationException("Falta el paquete de instalación.");
                resource.CopyTo(memory); payload = memory.ToArray();
            }
            string expected;
            using (Stream resource = assembly.GetManifestResourceStream("ScreenTime.Checksum"))
            using (StreamReader reader = new StreamReader(resource)) expected = reader.ReadToEnd().Trim();
            using (SHA256 sha = SHA256.Create())
            {
                string actual = BitConverter.ToString(sha.ComputeHash(payload)).Replace("-", "").ToLowerInvariant();
                if (actual != expected) throw new InvalidOperationException("El paquete no supera la verificación de integridad.");
            }
            using (MemoryStream memory = new MemoryStream(payload))
            using (ZipArchive archive = new ZipArchive(memory, ZipArchiveMode.Read))
            {
                foreach (ZipArchiveEntry entry in archive.Entries)
                {
                    string destination = Path.GetFullPath(Path.Combine(root, entry.FullName));
                    if (entry.FullName.IndexOf(':') >= 0 || !destination.StartsWith(root + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase))
                        throw new InvalidOperationException("Ruta no válida en el paquete.");
                    if (String.IsNullOrEmpty(entry.Name)) { Directory.CreateDirectory(destination); continue; }
                    Directory.CreateDirectory(Path.GetDirectoryName(destination));
                    using (Stream source = entry.Open())
                    using (FileStream target = new FileStream(destination, FileMode.CreateNew, FileAccess.Write)) source.CopyTo(target);
                }
            }
            foreach (string name in new string[] { "screen-time.exe", "LICENSE", "README.md", "PRIVACY.md", "CHARTJS-LICENSE.txt", "tools/install-user.ps1", "tools/install-core.ps1", "tools/uninstall-user.ps1", "tools/uninstall-core.ps1", "docs/ARQUITECTURA.md", "docs/VALIDACION.md", "docs/DEPENDENCIAS.md", "docs/PUBLICAR.md" })
                if (!File.Exists(Path.Combine(root, name))) throw new InvalidOperationException("Paquete incompleto: " + name);
            if (FileVersionInfo.GetVersionInfo(Path.Combine(root, "screen-time.exe")).FileDescription != "Screen Time")
                throw new InvalidOperationException("El ejecutable del paquete no corresponde a Screen Time.");
            string powershell = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.System), "WindowsPowerShell", "v1.0", "powershell.exe");
            string runner = Path.Combine(root, "run.ps1");
            string action = verifyOnly
                ? "$null = Get-FileHash -LiteralPath (Join-Path $PSScriptRoot 'screen-time.exe')\r\n"
                : "& (Join-Path $PSScriptRoot 'tools/install-user.ps1') -SourceDirectory $PSScriptRoot\r\n";
            File.WriteAllText(runner, "$ErrorActionPreference = 'Stop'\r\n[Console]::OutputEncoding = New-Object Text.UTF8Encoding($false)\r\n$OutputEncoding = [Console]::OutputEncoding\r\n" + action, new UTF8Encoding(true));
            ProcessStartInfo start = new ProcessStartInfo(powershell,
                "-NoProfile -NonInteractive -ExecutionPolicy Bypass -File " + Quote(runner));
            start.UseShellExecute = false;
            start.CreateNoWindow = true;
            start.RedirectStandardOutput = true;
            start.RedirectStandardError = true;
            start.StandardOutputEncoding = Encoding.UTF8;
            start.StandardErrorEncoding = Encoding.UTF8;
            // A parent running PowerShell 7 can export module paths incompatible
            // with Windows PowerShell 5.1. Use its own trusted built-in modules.
            start.EnvironmentVariables["PSModulePath"] = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.System), "WindowsPowerShell", "v1.0", "Modules");
            start.WorkingDirectory = root;
            using (Process process = Process.Start(start))
            {
                Task<string> output = process.StandardOutput.ReadToEndAsync();
                Task<string> errors = process.StandardError.ReadToEndAsync();
                process.WaitForExit();
                Task.WaitAll(output, errors);
                if (process.ExitCode != 0)
                    throw new InvalidOperationException("No se pudo completar la instalación.\r\n\r\n" + errors.Result + "\r\n" + output.Result);
            }
        }
        finally
        {
            // Only remove this invocation's generated child of the temp directory.
            string resolved = Path.GetFullPath(root);
            if (Directory.Exists(resolved) && Directory.GetParent(resolved).FullName.TrimEnd(Path.DirectorySeparatorChar) == temp && Path.GetFileName(resolved).StartsWith("screen-time-setup-", StringComparison.Ordinal))
            {
                try { Directory.Delete(resolved, true); } catch (IOException) { } catch (UnauthorizedAccessException) { }
            }
        }
    }
}

internal sealed class SetupWindow : Form
{
    private readonly Label status;
    private readonly Button install;
    private readonly Button cancel;
    private readonly ProgressBar progress;
    private bool busy;
    private bool installed;

    internal SetupWindow()
    {
        Text = "Instalar Screen Time";
        ClientSize = new Size(520, 285);
        Font = new Font("Segoe UI", 10);
        AutoScaleDimensions = new SizeF(96, 96);
        AutoScaleMode = AutoScaleMode.Dpi;
        FormBorderStyle = FormBorderStyle.FixedDialog;
        MaximizeBox = false;
        StartPosition = FormStartPosition.CenterScreen;
        BackColor = SystemColors.Window;
        Icon = Icon.ExtractAssociatedIcon(Application.ExecutablePath);
        Label title = new Label { Text = "Screen Time", Font = new Font("Segoe UI", 22, FontStyle.Bold), Location = new Point(24, 20), AutoSize = true };
        status = new Label { Text = "Instalá Screen Time para seguir el uso de tus aplicaciones.\r\n\r\nLa actualización conserva tu historial y configuración.\r\nSe instala para tu usuario y requiere WebView2.", Location = new Point(26, 78), Size = new Size(468, 100) };
        progress = new ProgressBar { Location = new Point(26, 187), Size = new Size(468, 12), Visible = false, Style = ProgressBarStyle.Marquee };
        install = new Button { Text = "Instalar", Location = new Point(286, 227), Size = new Size(100, 34) };
        cancel = new Button { Text = "Cancelar", Location = new Point(394, 227), Size = new Size(100, 34) };
        Controls.AddRange(new Control[] { title, status, progress, install, cancel });
        AcceptButton = install;
        CancelButton = cancel;
        cancel.Click += delegate { Close(); };
        install.Click += Install;
        FormClosing += delegate(object sender, FormClosingEventArgs e) { if (busy) e.Cancel = true; };
    }

    private async void Install(object sender, EventArgs args)
    {
        if (installed)
        {
            try { Process.Start(new ProcessStartInfo(Setup.InstalledExecutable, "--dashboard") { UseShellExecute = true }); Close(); }
            catch (Exception error) { MessageBox.Show(this, error.Message, "Screen Time", MessageBoxButtons.OK, MessageBoxIcon.Error); }
            return;
        }
        busy = true; install.Enabled = false; cancel.Enabled = false; progress.Visible = true;
        status.Text = "Instalando Screen Time…\r\n\r\nSe guardará el seguimiento activo y se respaldarán los datos antes de actualizar.";
        try
        {
            await Task.Run(() => Setup.Deploy(false));
            installed = true;
            Text = "Screen Time instalado";
            status.Text = "La instalación se completó.\r\n\r\nScreen Time está en ejecución. Podés abrirlo desde el acceso directo o el icono de la bandeja.";
            install.Text = "Abrir"; cancel.Text = "Cerrar";
        }
        catch (Exception error)
        {
            status.Text = "La instalación no se completó. Podés reintentar después de resolver el problema indicado.";
            MessageBox.Show(this, error.Message, "No se pudo instalar Screen Time", MessageBoxButtons.OK, MessageBoxIcon.Error);
        }
        finally { busy = false; install.Enabled = true; cancel.Enabled = true; progress.Visible = false; }
    }
}
