Add-Type -AssemblyName PresentationFramework, PresentationCore, WindowsBase, Microsoft.VisualBasic

[xml]$xaml = @'
<Window xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
        xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
        Title="kripusek.top — GTA VI Launcher"
        Height="840" Width="1100"
        WindowStartupLocation="CenterScreen"
        ResizeMode="CanMinimize"
        Background="#F5F5DC"
        Foreground="#111111"
        FontFamily="Segoe UI, Arial, sans-serif">
    
    <Window.Resources>
        <!-- kripusek.top Palette (no AI dark-mode gradients/glows) -->
        <SolidColorBrush x:Key="CardGradient" Color="#FFFDF5"/>
        <SolidColorBrush x:Key="CardHoverGradient" Color="#7FFFD4"/>
        <SolidColorBrush x:Key="LaunchButtonGradient" Color="#FA017D"/>

        <!-- Custom Card RadioButton Style (kripusek.top light card + cyan active) -->
        <Style x:Key="PresetCardStyle" TargetType="RadioButton">
            <Setter Property="Background" Value="{StaticResource CardGradient}"/>
            <Setter Property="BorderBrush" Value="#660085"/>
            <Setter Property="BorderThickness" Value="2"/>
            <Setter Property="Foreground" Value="#111111"/>
            <Setter Property="Margin" Value="0,0,0,10"/>
            <Setter Property="Cursor" Value="Hand"/>
            <Setter Property="Template">
                <Setter.Value>
                    <ControlTemplate TargetType="RadioButton">
                        <Border x:Name="border"
                                Background="{TemplateBinding Background}"
                                BorderBrush="{TemplateBinding BorderBrush}"
                                BorderThickness="{TemplateBinding BorderThickness}"
                                CornerRadius="5"
                                Padding="14,10">
                            <ContentPresenter />
                        </Border>
                        <ControlTemplate.Triggers>
                            <Trigger Property="IsMouseOver" Value="True">
                                <Setter TargetName="border" Property="Background" Value="#7FFFD4"/>
                                <Setter TargetName="border" Property="BorderBrush" Value="#660085"/>
                            </Trigger>
                            <Trigger Property="IsChecked" Value="True">
                                <Setter TargetName="border" Property="BorderBrush" Value="#FA017D"/>
                                <Setter TargetName="border" Property="BorderThickness" Value="3"/>
                                <Setter TargetName="border" Property="Background" Value="#00FFFF"/>
                            </Trigger>
                        </ControlTemplate.Triggers>
                    </ControlTemplate>
                </Setter.Value>
            </Setter>
        </Style>

        <!-- Action Button Style (kripusek.top .button2) -->
        <Style x:Key="ActionButtonStyle" TargetType="Button">
            <Setter Property="Background" Value="#DF1FDF"/>
            <Setter Property="BorderBrush" Value="#B700FF"/>
            <Setter Property="BorderThickness" Value="3"/>
            <Setter Property="Foreground" Value="#FFFFFF"/>
            <Setter Property="FontSize" Value="13"/>
            <Setter Property="FontWeight" Value="Bold"/>
            <Setter Property="Padding" Value="14,9"/>
            <Setter Property="Cursor" Value="Hand"/>
            <Setter Property="Template">
                <Setter.Value>
                    <ControlTemplate TargetType="Button">
                        <Border x:Name="btnBorder"
                                Background="{TemplateBinding Background}"
                                BorderBrush="{TemplateBinding BorderBrush}"
                                BorderThickness="{TemplateBinding BorderThickness}"
                                CornerRadius="5"
                                Padding="{TemplateBinding Padding}">
                            <ContentPresenter HorizontalAlignment="Center" VerticalAlignment="Center"/>
                        </Border>
                        <ControlTemplate.Triggers>
                            <Trigger Property="IsMouseOver" Value="True">
                                <Setter TargetName="btnBorder" Property="Background" Value="#007185"/>
                            </Trigger>
                            <Trigger Property="IsPressed" Value="True">
                                <Setter TargetName="btnBorder" Property="Background" Value="#004F5E"/>
                            </Trigger>
                        </ControlTemplate.Triggers>
                    </ControlTemplate>
                </Setter.Value>
            </Setter>
        </Style>
    </Window.Resources>

    <Grid Margin="24">
        <Grid.RowDefinitions>
            <RowDefinition Height="Auto"/> <!-- Header -->
            <RowDefinition Height="Auto"/> <!-- Status Pills -->
            <RowDefinition Height="*"/>    <!-- Main Body: Presets & Controls -->
            <RowDefinition Height="Auto"/> <!-- Action Bar / Big Button -->
        </Grid.RowDefinitions>

        <!-- 1. Header (kripusek.top .logo style) -->
        <Grid Grid.Row="0" Margin="0,0,0,14">
            <Grid.ColumnDefinitions>
                <ColumnDefinition Width="*"/>
                <ColumnDefinition Width="Auto"/>
            </Grid.ColumnDefinitions>

            <StackPanel Orientation="Horizontal" VerticalAlignment="Center">
                <Border Background="#660085" Padding="3" CornerRadius="4">
                    <Border Background="#00FFFF" BorderBrush="#660085" BorderThickness="2" CornerRadius="2" Padding="18,6">
                        <TextBlock Text="Kripusek.top • GTA VI" FontSize="24" FontWeight="Black" Foreground="#111111"/>
                    </Border>
                </Border>
                <Border Background="#00B7FF" CornerRadius="4" Margin="14,0,0,0" Padding="10,5" VerticalAlignment="Center" BorderBrush="#660085" BorderThickness="2">
                    <TextBlock Text="VI CAUSEWAY LAUNCHER" FontSize="12" FontWeight="Bold" Foreground="#111111"/>
                </Border>
            </StackPanel>

            <StackPanel Grid.Column="1" Orientation="Horizontal" VerticalAlignment="Center">
                <Border Background="#FFFDF5" BorderBrush="#660085" BorderThickness="2" CornerRadius="5" Padding="12,6" Margin="0,0,8,0">
                    <StackPanel Orientation="Horizontal">
                        <TextBlock Text="🎮 GRA: " FontWeight="Bold" Foreground="#333333" FontSize="12"/>
                        <TextBlock Text="GTA VI 1.0.2.1" FontWeight="Bold" Foreground="#008800" FontSize="12"/>
                    </StackPanel>
                </Border>
                <Border Background="#FFFDF5" BorderBrush="#660085" BorderThickness="2" CornerRadius="5" Padding="12,6">
                    <StackPanel Orientation="Horizontal">
                        <TextBlock Text="👤 USER: " FontWeight="Bold" Foreground="#333333" FontSize="12"/>
                        <TextBlock Text="kripusek" FontWeight="Bold" Foreground="#660085" FontSize="12"/>
                    </StackPanel>
                </Border>
            </StackPanel>
        </Grid>

        <!-- 2. System Status Bar -->
        <Border Grid.Row="1" Background="#EFEBD6" BorderBrush="#660085" BorderThickness="2" CornerRadius="5" Padding="14,10" Margin="0,0,0,16">
            <Grid>
                <Grid.ColumnDefinitions>
                    <ColumnDefinition Width="*"/>
                    <ColumnDefinition Width="*"/>
                    <ColumnDefinition Width="*"/>
                    <ColumnDefinition Width="*"/>
                </Grid.ColumnDefinitions>

                <StackPanel Grid.Column="0">
                    <TextBlock Text="KARTA GRAFICZNA" FontSize="10" FontWeight="Bold" Foreground="#660085"/>
                    <TextBlock x:Name="txtGPU" Text="NVIDIA RTX 3060 Ti" FontSize="13" FontWeight="Bold" Foreground="#111111" Margin="0,2,0,0"/>
                </StackPanel>

                <StackPanel Grid.Column="1">
                    <TextBlock Text="ROZMIAR FALI SHADERÓW" FontSize="10" FontWeight="Bold" Foreground="#660085"/>
                    <TextBlock Text="Wave32 (PR #578 Safe)" FontSize="13" FontWeight="Bold" Foreground="#007700" Margin="0,2,0,0"/>
                </StackPanel>

                <StackPanel Grid.Column="2">
                    <TextBlock Text="STAN KOMPILATORA" FontSize="10" FontWeight="Bold" Foreground="#660085"/>
                    <TextBlock Text="causeway-integration (OK)" FontSize="13" FontWeight="Bold" Foreground="#995500" Margin="0,2,0,0"/>
                </StackPanel>

                <StackPanel Grid.Column="3">
                    <TextBlock Text="TESTY SHADERÓW / GPU" FontSize="10" FontWeight="Bold" Foreground="#660085"/>
                    <TextBlock Text="243/243 ZALICZONE" FontSize="13" FontWeight="Bold" Foreground="#660085" Margin="0,2,0,0"/>
                </StackPanel>
            </Grid>
        </Border>

        <!-- 3. Main Body -->
        <Grid Grid.Row="2" Margin="0,0,0,16">
            <Grid.ColumnDefinitions>
                <ColumnDefinition Width="1.35*"/> <!-- Presets -->
                <ColumnDefinition Width="*"/>    <!-- Options & Log Preview -->
            </Grid.ColumnDefinitions>

            <!-- Presets Section -->
            <Border Grid.Column="0" Background="#EFEBD6" BorderBrush="#660085" BorderThickness="2" CornerRadius="5" Padding="18" Margin="0,0,12,0">
                <ScrollViewer VerticalScrollBarVisibility="Auto">
                    <StackPanel Orientation="Vertical">
                        <TextBlock Text="✅ PROFIL URUCHOMIENIA" FontSize="13" FontWeight="Bold" Foreground="#660085" Margin="0,0,0,10"/>

                        <!-- Preset 1: VIctory-NVIDIA-RT rt-test-11 (da309b8) -->
                        <RadioButton x:Name="optNvidiaRt" Style="{StaticResource PresetCardStyle}" IsChecked="True">
                            <Grid>
                                <Grid.ColumnDefinitions>
                                    <ColumnDefinition Width="Auto"/>
                                    <ColumnDefinition Width="*"/>
                                    <ColumnDefinition Width="Auto"/>
                                </Grid.ColumnDefinitions>
                                <TextBlock Text="⚡" FontSize="18" VerticalAlignment="Center" Margin="0,0,12,0"/>
                                <StackPanel Grid.Column="1">
                                    <TextBlock Text="VIctory-NVIDIA-RT rt-test-11 (da309b8 • Max FPS)" FontSize="14" FontWeight="Bold" Foreground="#111111"/>
                                    <TextBlock Text="NVIDIA RTX: Series S + DLSS Quality + -Fast (bez FG, max prawdziwe FPS) + F1/F3" FontSize="11" Foreground="#333333" Margin="0,2,0,0"/>
                                </StackPanel>
                                <Border Grid.Column="2" Background="#DF1FDF" BorderBrush="#111111" BorderThickness="1" CornerRadius="4" Padding="6,3" VerticalAlignment="Center">
                                    <TextBlock Text="NVIDIA" FontSize="9" FontWeight="Black" Foreground="#FFFFFF"/>
                                </Border>
                            </Grid>
                        </RadioButton>

                        <!-- Preset 2: VIctory-Belobug-AMD -->
                        <RadioButton x:Name="optBelobug" Style="{StaticResource PresetCardStyle}">
                            <Grid>
                                <Grid.ColumnDefinitions>
                                    <ColumnDefinition Width="Auto"/>
                                    <ColumnDefinition Width="*"/>
                                    <ColumnDefinition Width="Auto"/>
                                </Grid.ColumnDefinitions>
                                <TextBlock Text="🔴" FontSize="18" VerticalAlignment="Center" Margin="0,0,12,0"/>
                                <StackPanel Grid.Column="1">
                                    <TextBlock Text="VIctory-Belobug-AMD (Radeon RDNA2/3/4 • Series S)" FontSize="14" FontWeight="Bold" Foreground="#111111"/>
                                    <TextBlock Text="Dedykowany fork pod AMD Radeon (STRETCH=1, ASYNC_SUBMIT=1, HOIST_CLAMPS=7, quad-reads)" FontSize="11" Foreground="#333333" Margin="0,2,0,0"/>
                                </StackPanel>
                                <Border Grid.Column="2" Background="#FA017D" BorderBrush="#111111" BorderThickness="1" CornerRadius="4" Padding="6,3" VerticalAlignment="Center">
                                    <TextBlock Text="AMD" FontSize="9" FontWeight="Black" Foreground="#FFFFFF"/>
                                </Border>
                            </Grid>
                        </RadioButton>

                    </StackPanel>
                </ScrollViewer>
            </Border>

            <!-- Tweaks & Diagnostics Section -->
            <Border Grid.Column="1" Background="#EFEBD6" BorderBrush="#660085" BorderThickness="2" CornerRadius="5" Padding="18">
                <Grid>
                    <Grid.RowDefinitions>
                        <RowDefinition Height="Auto"/>
                        <RowDefinition Height="Auto"/>
                        <RowDefinition Height="Auto"/>
                        <RowDefinition Height="*"/>
                    </Grid.RowDefinitions>

                    <TextBlock Grid.Row="0" Text="OPCJE SILNIKA I GRAFIKI" FontSize="14" FontWeight="Bold" Foreground="#660085" Margin="0,0,0,12"/>

                    <StackPanel Grid.Row="1" Margin="0,0,0,12">
                        <CheckBox x:Name="chkClzero" Content="Aktywne buforowanie CLZERO (Wysoki FPS + Ochrona RAM)" IsChecked="True" Foreground="#111111" Margin="0,0,0,8" Cursor="Hand"/>
                        <CheckBox x:Name="chkSafeSubmit" Content="Tryb Safe Submit (Tylko diagnostyka; obniża FPS!)" IsChecked="False" Foreground="#444444" Margin="0,0,0,8" Cursor="Hand"/>
                        <CheckBox x:Name="chkDred" Content="D3D12 DRED &amp; Breadcrumbs (Narzut debuggera)" IsChecked="False" Foreground="#444444" Margin="0,0,0,8" Cursor="Hand"/>
                        <CheckBox x:Name="chkMaxWorkers" Content="Limit workerów shaderów: 2 (Dla słabszych CPU)" IsChecked="False" Foreground="#444444" Margin="0,0,0,8" Cursor="Hand"/>
                        <CheckBox x:Name="chkShowConsole" Content="Osobne okno konsoli (bez podglądu stdout w analizatorze)" IsChecked="False" Foreground="#111111" Margin="0,0,0,8" Cursor="Hand"/>
                    </StackPanel>

                    <!-- Quick Tools & Actions -->
                    <Border Grid.Row="2" Background="#F5F5DC" BorderBrush="#660085" BorderThickness="2" CornerRadius="5" Padding="10" Margin="0,0,0,10">
                        <StackPanel>
                            <TextBlock Text="SZYBKIE NARZĘDZIA" FontSize="11" FontWeight="Bold" Foreground="#660085" Margin="0,0,0,6"/>
                            <Grid Margin="0,0,0,6">
                                <Grid.ColumnDefinitions>
                                    <ColumnDefinition Width="*"/>
                                    <ColumnDefinition Width="*"/>
                                </Grid.ColumnDefinitions>
                                <Button x:Name="btnViewLogs" Grid.Column="0" Content="📋 Ostatnie Logi" Style="{StaticResource ActionButtonStyle}" Margin="0,0,4,0"/>
                                <Button x:Name="btnClearCache" Grid.Column="1" Content="🧹 Wyczyść Cache" Style="{StaticResource ActionButtonStyle}" Margin="4,0,0,0"/>
                            </Grid>
                            <Grid Margin="0,0,0,6">
                                <Grid.ColumnDefinitions>
                                    <ColumnDefinition Width="*"/>
                                    <ColumnDefinition Width="*"/>
                                </Grid.ColumnDefinitions>
                                <Button x:Name="btnOpenFolder" Grid.Column="0" Content="📁 Folder Gry" Style="{StaticResource ActionButtonStyle}" Margin="0,0,4,0"/>
                                <Button x:Name="btnCheckStatus" Grid.Column="1" Content="🔍 Test Binariów" Style="{StaticResource ActionButtonStyle}" Margin="4,0,0,0"/>
                            </Grid>
                            <Button x:Name="btnSteamSync" Content="🎮 Dodaj / Odśwież w Steam" Style="{StaticResource ActionButtonStyle}" Margin="0,0,0,0"/>
                            
                            <TextBlock x:Name="txtToolStatus" Text="System gotowy. Wybierz profil i kliknij ODPAL." FontSize="11" FontWeight="Bold" Foreground="#007700" Margin="0,8,0,0" TextWrapping="Wrap"/>
                        </StackPanel>
                    </Border>

                    <!-- Real-Time Live Resource Monitor -->
                    <Border Grid.Row="3" Background="#F5F5DC" BorderBrush="#660085" BorderThickness="2" CornerRadius="5" Padding="10">
                        <Grid>
                            <Grid.RowDefinitions>
                                <RowDefinition Height="Auto"/>
                                <RowDefinition Height="Auto"/>
                                <RowDefinition Height="Auto"/>
                                <RowDefinition Height="Auto"/>
                                <RowDefinition Height="*"/>
                            </Grid.RowDefinitions>

                            <Grid Grid.Row="0" Margin="0,0,0,6">
                                <TextBlock Text="📊 MONITOR ZASOBÓW NA ŻYWO" FontSize="11" FontWeight="Bold" Foreground="#660085" HorizontalAlignment="Left"/>
                                <TextBlock x:Name="txtLiveStage" Text="⏸ Oczekiwanie na start" FontSize="11" FontWeight="Bold" Foreground="#333333" HorizontalAlignment="Right"/>
                            </Grid>

                            <Grid Grid.Row="1" Margin="0,0,0,6">
                                <Grid.ColumnDefinitions>
                                    <ColumnDefinition Width="*"/>
                                    <ColumnDefinition Width="*"/>
                                </Grid.ColumnDefinitions>
                                <StackPanel Grid.Column="0" Margin="0,0,6,0">
                                    <TextBlock Text="FPS / KLATKA" FontSize="9" FontWeight="Bold" Foreground="#660085"/>
                                    <TextBlock x:Name="txtLiveFps" Text="--" FontSize="12" FontWeight="Bold" Foreground="#007700"/>
                                </StackPanel>
                                <StackPanel Grid.Column="1">
                                    <TextBlock Text="CPU GRY" FontSize="9" FontWeight="Bold" Foreground="#660085"/>
                                    <TextBlock x:Name="txtLiveCpu" Text="--" FontSize="12" FontWeight="Bold" Foreground="#0055AA"/>
                                </StackPanel>
                            </Grid>

                            <Grid Grid.Row="2" Margin="0,0,0,6">
                                <Grid.ColumnDefinitions>
                                    <ColumnDefinition Width="*"/>
                                    <ColumnDefinition Width="*"/>
                                </Grid.ColumnDefinitions>
                                <StackPanel Grid.Column="0" Margin="0,0,6,0">
                                    <TextBlock Text="RAM PROCESU / WOLNE" FontSize="9" FontWeight="Bold" Foreground="#660085"/>
                                    <TextBlock x:Name="txtLiveRam" Text="--" FontSize="12" FontWeight="Bold" Foreground="#660085"/>
                                </StackPanel>
                                <StackPanel Grid.Column="1">
                                    <TextBlock Text="VRAM DEDYKOWANY" FontSize="9" FontWeight="Bold" Foreground="#660085"/>
                                    <TextBlock x:Name="txtLiveVram" Text="--" FontSize="12" FontWeight="Bold" Foreground="#990066"/>
                                </StackPanel>
                            </Grid>

                            <Grid Grid.Row="3" Margin="0,0,0,6">
                                <Grid.ColumnDefinitions>
                                    <ColumnDefinition Width="*"/>
                                    <ColumnDefinition Width="*"/>
                                </Grid.ColumnDefinitions>
                                <StackPanel Grid.Column="0" Margin="0,0,6,0">
                                    <TextBlock Text="SHADERY / POTOKI" FontSize="9" FontWeight="Bold" Foreground="#660085"/>
                                    <TextBlock x:Name="txtLiveShaders" Text="0" FontSize="12" FontWeight="Bold" Foreground="#995500"/>
                                </StackPanel>
                                <StackPanel Grid.Column="1">
                                    <TextBlock Text="WARN / BŁĘDY" FontSize="9" FontWeight="Bold" Foreground="#660085"/>
                                    <TextBlock x:Name="txtLiveErrors" Text="0 / 0" FontSize="12" FontWeight="Bold" Foreground="#333333"/>
                                </StackPanel>
                            </Grid>

                            <TextBox x:Name="txtLiveLog" Grid.Row="4"
                                     IsReadOnly="True"
                                     Background="#FFFDF5"
                                     Foreground="#111111"
                                     BorderBrush="#660085"
                                     BorderThickness="1"
                                     FontFamily="Consolas"
                                     FontSize="10"
                                     VerticalScrollBarVisibility="Auto"
                                     HorizontalScrollBarVisibility="Auto"
                                     TextWrapping="NoWrap"
                                     Text="Analizator gotowy. Po uruchomieniu gry zobaczysz tu na żywo etap, FPS, RAM, kompilację shaderów i błędy."/>
                        </Grid>
                    </Border>
                </Grid>
            </Border>
        </Grid>

        <!-- 4. Giant Launch Bar -->
        <Grid Grid.Row="3">
            <Grid.ColumnDefinitions>
                <ColumnDefinition Width="*"/>
                <ColumnDefinition Width="Auto"/>
                <ColumnDefinition Width="Auto"/>
            </Grid.ColumnDefinitions>

            <Button x:Name="btnLaunch" Grid.Column="0" Height="68" Margin="0,0,12,0" Cursor="Hand" BorderThickness="0">
                <Button.Template>
                    <ControlTemplate TargetType="Button">
                        <Border x:Name="launchBorder"
                                Background="{StaticResource LaunchButtonGradient}"
                                BorderBrush="#00FF00"
                                BorderThickness="3"
                                CornerRadius="5">
                            <StackPanel Orientation="Horizontal" HorizontalAlignment="Center" VerticalAlignment="Center">
                                <TextBlock Text="▶" FontSize="26" Foreground="#FFFFFF" VerticalAlignment="Center" Margin="0,0,14,0"/>
                                <StackPanel VerticalAlignment="Center">
                                    <TextBlock Text="ODPAL GTA VI" FontSize="20" FontWeight="Black" Foreground="#FFFFFF"/>
                                    <TextBlock Text="kripusek.top • Uruchom Grand Theft Auto VI" FontSize="11" Foreground="#FFFFFF"/>
                                </StackPanel>
                            </StackPanel>
                        </Border>
                        <ControlTemplate.Triggers>
                            <Trigger Property="IsMouseOver" Value="True">
                                <Setter TargetName="launchBorder" Property="Background" Value="#660085"/>
                            </Trigger>
                            <Trigger Property="IsPressed" Value="True">
                                <Setter TargetName="launchBorder" Property="Opacity" Value="0.85"/>
                            </Trigger>
                        </ControlTemplate.Triggers>
                    </ControlTemplate>
                </Button.Template>
            </Button>

            <Border Grid.Column="1" Height="68" Background="#00B7FF" BorderBrush="#660085" BorderThickness="3" CornerRadius="5" Padding="14,0" Margin="0,0,12,0">
                <CheckBox x:Name="chkSkipMovie" Content="Skip Movie / Intro" IsChecked="False" Foreground="#111111" FontWeight="Bold" FontSize="12" VerticalAlignment="Center" Cursor="Hand" ToolTip="Auto-FastBoot/SkipCutscenes dla ScarlettPort oraz automatyczne wysyłanie Enter do okna gry przez pierwsze 120s dla pozostałych portów"/>
            </Border>

            <Button x:Name="btnExit" Grid.Column="2" Width="100" Height="68" Content="WYJŚCIE" Style="{StaticResource ActionButtonStyle}" FontSize="14" FontWeight="Bold"/>
        </Grid>
    </Grid>
</Window>
'@

# Load XAML
$reader = New-Object System.Xml.XmlNodeReader $xaml
$window = [System.Windows.Markup.XamlReader]::Load($reader)

# Element References
$optNvidiaRt = $window.FindName("optNvidiaRt")
$optBelobug = $window.FindName("optBelobug")

$chkClzero = $window.FindName("chkClzero")
$chkSafeSubmit = $window.FindName("chkSafeSubmit")
$chkDred = $window.FindName("chkDred")
$chkMaxWorkers = $window.FindName("chkMaxWorkers")
$chkShowConsole = $window.FindName("chkShowConsole")
$chkSkipMovie = $window.FindName("chkSkipMovie")

$btnViewLogs = $window.FindName("btnViewLogs")
$btnClearCache = $window.FindName("btnClearCache")
$btnOpenFolder = $window.FindName("btnOpenFolder")
$btnCheckStatus = $window.FindName("btnCheckStatus")
$btnSteamSync = $window.FindName("btnSteamSync")
$btnLaunch = $window.FindName("btnLaunch")
$btnExit = $window.FindName("btnExit")
$txtToolStatus = $window.FindName("txtToolStatus")
$txtLiveStage = $window.FindName("txtLiveStage")
$txtLiveFps = $window.FindName("txtLiveFps")
$txtLiveCpu = $window.FindName("txtLiveCpu")
$txtLiveRam = $window.FindName("txtLiveRam")
$txtLiveVram = $window.FindName("txtLiveVram")
$txtLiveShaders = $window.FindName("txtLiveShaders")
$txtLiveErrors = $window.FindName("txtLiveErrors")
$txtLiveLog = $window.FindName("txtLiveLog")

# Populate GPU Info
try {
    $gpu = Get-CimInstance Win32_VideoController | Where-Object { $_.Name -like "*NVIDIA*" -or $_.Name -like "*AMD*" -or $_.Name -like "*Intel*" } | Select-Object -First 1
    if ($gpu) {
        $txtGPU = $window.FindName("txtGPU")
        if ($txtGPU) { $txtGPU.Text = "$($gpu.Name)" }
    }
} catch {}

$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$gameDir = Join-Path $root 'GTA6.Xbox.v1.0.2.1-CYBERLEEK'
$gameExe = Join-Path $gameDir 'game_scarlett_master.exe'
$nvidiaRtDir = Join-Path $root 'victory-nvidia-rt'
$nvidiaRtExe = Join-Path $nvidiaRtDir 'target\release\victory.exe'
$nvidiaRtScript = Join-Path $nvidiaRtDir 'scripts\play-foxhound.ps1'
$belobugDir = Join-Path $root 'victory-belobug'
$belobugExe = Join-Path $belobugDir 'target\release\victory.exe'
$dataDir = Join-Path $root 'victory-data'
$logDir = Join-Path $dataDir 'launcher-logs'
$steamScript = Join-Path $root 'dodaj_do_steama.py'
$jobs = @{}

function Set-Status($Message, $Color = 'Cyan') {
    $txtToolStatus.Text = $Message
    $txtToolStatus.Foreground = $Color
}

function Require-File($Path) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "Brakuje pliku: $Path"
    }
}

function Start-LauncherProcess($File, $Arguments, $Directory, $Environment = @{}, [switch]$Hidden) {
    $start = @{
        FilePath = $File
        WorkingDirectory = $Directory
        ArgumentList = ($Arguments | ForEach-Object { '"' + $_ + '"' }) -join ' '
        PassThru = $true
    }
    if ($Hidden) {
        $null = New-Item -ItemType Directory -Path $logDir -Force
        $prefix = Join-Path $logDir ([guid]::NewGuid().ToString())
        $start.WindowStyle = 'Hidden'
        $start.RedirectStandardOutput = "$prefix.stdout.log"
        $start.RedirectStandardError = "$prefix.stderr.log"
    }

    # Windows PowerShell 5.1 inherits the parent environment at process creation.
    $previous = @{}
    try {
        foreach ($name in $Environment.Keys) {
            $previous[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
            [Environment]::SetEnvironmentVariable($name, $Environment[$name], 'Process')
        }
        $process = Start-Process @start
    } finally {
        foreach ($name in $previous.Keys) {
            [Environment]::SetEnvironmentVariable($name, $previous[$name], 'Process')
        }
    }
    return @{ Process = $process; StdOut = $start.RedirectStandardOutput; StdErr = $start.RedirectStandardError }
}

function Get-LaunchPlan {
    $profile = 'VIctory Series S 1080p'
    $exe = $releaseExe
    $workDir = $victoryDir
    $console = 'series-s'
    $height = '1080'
    $environment = @{
        VICTORY_SHADER_LOOP_LIMIT = $null
        VICTORY_D3D12_DEBUG = $null
        VICTORY_D3D12_DRED = $null
        VICTORY_UNLOCK_FPS = $null
        VICTORY_BATCH_CLZERO = $null
        VICTORY_SAFE_SUBMIT = $null
        VICTORY_GPU_TIMING = $null
    }

    if ($optNvidiaRt.IsChecked) {
        Require-File $nvidiaRtExe
        Require-File $nvidiaRtScript
        $profile = 'VIctory-NVIDIA-RT rt-test-11 (da309b8 • Series S + DLSS Quality)'
        $exe = Join-Path $PSHOME 'powershell.exe'
        $workDir = $nvidiaRtDir
        $environment.VICTORY_REFLEX = 'boost'
        $environment.VICTORY_FULLSCREEN = '1'
        $environment.VICTORY_HDR = '0'
        $arguments = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $nvidiaRtScript,
            '-Exe', $gameExe, '-SeriesS', '-Dlss', 'quality', '-NoFrameGenReady', '-Fast', '-Build', 'release')
    } elseif ($optBelobug.IsChecked) {
        Require-File $belobugExe
        $profile = 'VIctory-Belobug-AMD (Radeon • Series S)'
        $exe = $belobugExe
        $workDir = $belobugDir
        $environment.VICTORY_AMBIENT = '0.6'
        $environment.VICTORY_AMBIENT_REFLECT = '0.5'
        $environment.VICTORY_HOIST_CLAMPS = '7'
        $environment.VICTORY_ASYNC_SUBMIT = '1'
        $environment.VICTORY_STRETCH = '1'
        $arguments = @('run', $gameExe, '--execute', '--console', 'series-s')
    } else {
        throw 'Wybierz profil graficzny.'
    }
    return @{ Name = $profile; File = $exe; Arguments = $arguments; Directory = $workDir; Environment = $environment }
}

$refreshEngineOptions = {
    $chkClzero.IsEnabled = $false
    $chkSafeSubmit.IsEnabled = $false
    $chkMaxWorkers.IsEnabled = $false
    $chkDred.IsEnabled = $false
}
foreach ($preset in @($optNvidiaRt, $optBelobug)) {
    $preset.Add_Checked($refreshEngineOptions)
}
& $refreshEngineOptions

$liveState = @{
    StdOutPos = 0L
    StdErrPos = 0L
    Shaders = 0
    Warnings = 0
    Errors = 0
    Lines = New-Object System.Collections.Generic.List[string]
}

function Reset-LiveAnalyzer($ProfileName) {
    $liveState.StdOutPos = 0L
    $liveState.StdErrPos = 0L
    $liveState.RtLogPath = $null
    $liveState.RtLogPos = 0L
    $liveState.LastCpuTime = $null
    $liveState.LastCpuSample = $null
    $liveState.TickCount = 0
    $liveState.Shaders = 0
    $liveState.Warnings = 0
    $liveState.Errors = 0
    $liveState.StartedAt = [DateTime]::UtcNow
    $liveState.LastSkipSent = [DateTime]::MinValue
    $liveState.Lines.Clear()
    $txtLiveStage.Text = "⏳ Inicjalizacja ($ProfileName)..."
    $txtLiveStage.Foreground = '#660085'
    $txtLiveFps.Text = '--'
    $txtLiveCpu.Text = '--'
    $txtLiveRam.Text = '--'
    $txtLiveVram.Text = '--'
    $txtLiveShaders.Text = '0'
    $txtLiveErrors.Text = '0 / 0'
    $txtLiveErrors.Foreground = '#333333'
    $txtLiveLog.Text = "Uruchomiono $ProfileName. Oczekiwanie na zdarzenia..."
}

function Read-NewLogLines($Path, [ref]$Offset) {
    if (-not $Path -or -not (Test-Path -LiteralPath $Path -PathType Leaf)) { return @() }
    try {
        $fs = [System.IO.File]::Open($Path, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::ReadWrite)
        try {
            if ($fs.Length -le $Offset.Value) { return @() }
            $null = $fs.Seek($Offset.Value, [System.IO.SeekOrigin]::Begin)
            $sr = New-Object System.IO.StreamReader($fs)
            $text = $sr.ReadToEnd()
            $Offset.Value = $fs.Position
            if ([string]::IsNullOrEmpty($text)) { return @() }
            return ($text -split "`r?`n" | Where-Object { $_.Trim().Length -gt 0 })
        } finally {
            $fs.Dispose()
        }
    } catch {
        return @()
    }
}

if (-not ('KripusWin32' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class KripusWin32 {
    [DllImport("user32.dll")]
    public static extern bool PostMessage(IntPtr hWnd, uint Msg, IntPtr wParam, IntPtr lParam);
}
'@
}

$script:compInfo = New-Object Microsoft.VisualBasic.Devices.ComputerInfo

function Update-LiveAnalyzer {
    $liveState.TickCount++
    $proc = Get-Process -Name victory -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($proc) {
        $gb = [math]::Round($proc.WorkingSet64 / 1GB, 2)
        $freeGb = [math]::Round($script:compInfo.AvailablePhysicalMemory / 1GB, 1)
        $txtLiveRam.Text = "$gb GB (wolne $freeGb GB)"

        $nowCpu = [DateTime]::UtcNow
        $curCpu = $proc.TotalProcessorTime
        if ($liveState.LastCpuTime -and $liveState.LastCpuSample) {
            $dtMs = ($nowCpu - $liveState.LastCpuSample).TotalMilliseconds
            if ($dtMs -gt 100) {
                $dCpuMs = ($curCpu - $liveState.LastCpuTime).TotalMilliseconds
                $pct = [math]::Min(100.0, [math]::Round(($dCpuMs / ($dtMs * [Environment]::ProcessorCount)) * 100.0, 1))
                $txtLiveCpu.Text = "$pct %"
            }
        }
        $liveState.LastCpuTime = $curCpu
        $liveState.LastCpuSample = $nowCpu

        if (($liveState.TickCount % 2) -eq 0) {
            try {
                $cat = New-Object System.Diagnostics.PerformanceCounterCategory('GPU Process Memory')
                $inst = $cat.GetInstanceNames() | Where-Object { $_ -like "pid_$($proc.Id)_*" } | Select-Object -First 1
                if ($inst) {
                    $pc = New-Object System.Diagnostics.PerformanceCounter('GPU Process Memory', 'Dedicated Usage', $inst, $true)
                    $vramGb = [math]::Round($pc.NextValue() / 1GB, 2)
                    $pc.Dispose()
                    $txtLiveVram.Text = "$vramGb / 8.0 GB"
                }
            } catch {}
        }
    } elseif (-not $jobs.ContainsKey('game')) {
        $txtLiveRam.Text = '--'
        $txtLiveCpu.Text = '--'
        $txtLiveVram.Text = '--'
    }

    $job = $jobs['game']
    if (-not $job) { return }

    if ($chkSkipMovie.IsChecked -and $proc -and $proc.MainWindowHandle -ne [IntPtr]::Zero) {
        $now = [DateTime]::UtcNow
        if (-not $liveState.StartedAt) { $liveState.StartedAt = $now }
        if (-not $liveState.LastSkipSent) { $liveState.LastSkipSent = [DateTime]::MinValue }
        $elapsed = ($now - $liveState.StartedAt).TotalSeconds
        if ($elapsed -le 120 -and ($now - $liveState.LastSkipSent).TotalMilliseconds -ge 1500) {
            [void][KripusWin32]::PostMessage($proc.MainWindowHandle, 0x0100, [IntPtr]0x0D, [IntPtr]::Zero)
            [void][KripusWin32]::PostMessage($proc.MainWindowHandle, 0x0101, [IntPtr]0x0D, [IntPtr]::Zero)
            $liveState.LastSkipSent = $now
        }
    }

    $newLines = @()
    if ($job.StdOut) {
        $pos = $liveState.StdOutPos
        $newLines += Read-NewLogLines $job.StdOut ([ref]$pos)
        $liveState.StdOutPos = $pos
    }
    if ($job.StdErr) {
        $posErr = $liveState.StdErrPos
        $newLines += Read-NewLogLines $job.StdErr ([ref]$posErr)
        $liveState.StdErrPos = $posErr
    }
    if (-not $liveState.RtLogPath) {
        $rtDir = Join-Path $env:LOCALAPPDATA 'victory\logs'
        if (Test-Path -LiteralPath $rtDir) {
            $cand = Get-ChildItem -LiteralPath $rtDir -Filter '*.log' -ErrorAction SilentlyContinue |
                Where-Object { $_.LastWriteTimeUtc -ge $liveState.StartedAt.AddSeconds(-2) } |
                Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1
            if ($cand) { $liveState.RtLogPath = $cand.FullName }
        }
    }
    if ($liveState.RtLogPath) {
        $posRt = $liveState.RtLogPos
        $newLines += Read-NewLogLines $liveState.RtLogPath ([ref]$posRt)
        $liveState.RtLogPos = $posRt
    }

    if ($newLines.Count -eq 0) { return }

    foreach ($line in $newLines) {
        $clean = $line.Trim()
        if ($clean.Length -gt 220) { $clean = $clean.Substring(0, 220) + '...' }
        $liveState.Lines.Add($clean)
        if ($liveState.Lines.Count -gt 45) { $liveState.Lines.RemoveAt(0) }

        if ($clean -match '(?i)device removed|device lost|dxgi_error|panic|fatal|access violation|out of memory|crash') {
            $liveState.Errors++
            $txtLiveStage.Text = '💥 Błąd GPU / Crash!'
            $txtLiveStage.Foreground = '#F87171'
        } elseif ($clean -match '(?i)\bwarn\b|dropped|refused|timeout') {
            $liveState.Warnings++
        }

        if ($clean -match '(?i)pipeline|shader|precompile|translating|warm') {
            $liveState.Shaders++
            if ($liveState.Errors -eq 0) {
                $txtLiveStage.Text = '🔧 Kompilacja shaderów / potoków'
                $txtLiveStage.Foreground = '#E2B714'
            }
        } elseif ($clean -match '(?i)draws|world|prologue|directstorage|indirect|ambient') {
            if ($liveState.Errors -eq 0) {
                $txtLiveStage.Text = '🌍 Świat 3D / Rozgrywka'
                $txtLiveStage.Foreground = '#38EF7D'
            }
        } elseif ($clean -match '(?i)movie|bink|player|setup|menu|swapchain') {
            if ($liveState.Errors -eq 0 -and $txtLiveStage.Text -notlike '🌍*') {
                $txtLiveStage.Text = '🎬 Intro / Menu'
                $txtLiveStage.Foreground = '#60A5FA'
            }
        }

        if ($clean -match '(?i)(\d+(?:\.\d+)?)\s*fps') {
            $fpsVal = $Matches[1]
            if ($clean -match '(?i)(\d+(?:\.\d+)?)\s*ms') {
                $txtLiveFps.Text = "$fpsVal FPS ($($Matches[1]) ms)"
            } else {
                $txtLiveFps.Text = "$fpsVal FPS"
            }
        } elseif ($clean -match '(?i)(\d+(?:\.\d+)?)\s*ms(?:/frame|\s+a\s+frame)') {
            $msVal = [double]$Matches[1]
            if ($msVal -gt 0) {
                $calcFps = [math]::Round(1000.0 / $msVal, 1)
                $txtLiveFps.Text = "$calcFps FPS ($msVal ms)"
            }
        }
    }

    $txtLiveShaders.Text = "$($liveState.Shaders)"
    $txtLiveErrors.Text = "$($liveState.Warnings) / $($liveState.Errors)"
    if ($liveState.Errors -gt 0) { $txtLiveErrors.Foreground = '#F87171' }
    elseif ($liveState.Warnings -gt 0) { $txtLiveErrors.Foreground = '#F59E0B' }

    $txtLiveLog.Text = ($liveState.Lines -join [Environment]::NewLine)
    $txtLiveLog.ScrollToEnd()
}

$timer = New-Object System.Windows.Threading.DispatcherTimer
$timer.Interval = [TimeSpan]::FromMilliseconds(500)
$timer.Add_Tick({
    Update-LiveAnalyzer
    foreach ($name in @($jobs.Keys)) {
        $job = $jobs[$name]
        if (-not $job.Process.HasExited) { continue }
        $code = $job.Process.ExitCode
        $job.Process.Dispose()
        $jobs.Remove($name)
        if ($name -eq 'game') {
            $btnLaunch.IsEnabled = $true
            $btnClearCache.IsEnabled = $true
        } else {
            $btnSteamSync.IsEnabled = $true
        }

        if ($code -ne 0) {
            $message = "$($job.Name) zakończył działanie z kodem $code."
            if ($job.StdErr -and (Test-Path -LiteralPath $job.StdErr -PathType Leaf)) {
                $detail = Get-Content -LiteralPath $job.StdErr -Tail 1 -ErrorAction SilentlyContinue
                if ($detail) { $message += ' ' + $detail.Substring(0, [Math]::Min($detail.Length, 180)) }
            }
            Set-Status $message 'LightSalmon'
        } elseif ($name -eq 'steam') {
            Set-Status 'Zapisano skrót. Uruchom Steam ponownie, aby go zobaczyć.' 'LimeGreen'
        } else {
            Set-Status "$($job.Name) zakończył działanie (kod 0)."
        }
    }
})
$timer.Start()

$btnExit.Add_Click({ $window.Close() })
$window.Add_Closed({
    $timer.Stop()
    foreach ($job in $jobs.Values) { $job.Process.Dispose() }
})

$btnOpenFolder.Add_Click({
    try {
        Require-File $gameExe
        Start-Process -FilePath 'explorer.exe' -ArgumentList ('"' + $gameDir + '"')
        Set-Status 'Otwarto folder gry.'
    } catch { Set-Status $_.Exception.Message 'LightSalmon' }
})

$btnViewLogs.Add_Click({
    try {
        $logs = @(
            Get-ChildItem -LiteralPath $logDir, (Join-Path $scarlettDir 'logs') -Filter '*.log' -File -ErrorAction SilentlyContinue
            Get-Item -LiteralPath (Join-Path $dataDir 'data\7952987A\crashes.log'), (Join-Path $dataDir 'live_execution.log') -ErrorAction SilentlyContinue
        )
        $log = $logs | Where-Object Length -GT 0 | Sort-Object LastWriteTime -Descending | Select-Object -First 1
        if ($log) {
            Start-Process -FilePath 'notepad.exe' -ArgumentList ('"' + $log.FullName + '"')
            Set-Status "Otwarto $($log.Name)."
        } else {
            Set-Status 'Brak zapisanych logów.'
        }
    } catch { Set-Status $_.Exception.Message 'LightSalmon' }
})

$btnClearCache.Add_Click({
    try {
        if (Get-Process -Name victory -ErrorAction SilentlyContinue) {
            throw 'Zamknij VIctory przed czyszczeniem pamięci podręcznej.'
        }
        $count = 0
        $failed = 0
        foreach ($relative in @('cache\shaders', 'cache\pipelines', 'data\7952987A\cache', 'pipeline-cache')) {
            $path = Join-Path $dataDir $relative
            if (-not (Test-Path -LiteralPath $path -PathType Container)) { continue }
            foreach ($file in Get-ChildItem -LiteralPath $path -Recurse -File -Force) {
                try {
                    Remove-Item -LiteralPath $file.FullName -Force
                    $count++
                } catch { $failed++ }
            }
        }
        if ($failed) {
            Set-Status "Usunięto $count plików cache. Błąd przy $failed plikach." 'LightSalmon'
        } else {
            Set-Status "Usunięto $count plików cache shaderów i potoków."
        }
    } catch { Set-Status $_.Exception.Message 'LightSalmon' }
})

$btnCheckStatus.Add_Click({
    $checks = [ordered]@{
        'Plik gry' = $gameExe
        'VIctory-NVIDIA-RT rt-test-11' = $nvidiaRtExe
        'Skrypt play-foxhound.ps1' = $nvidiaRtScript
        'VIctory-Belobug-AMD' = $belobugExe
        'Skrypt Steam' = $steamScript
    }
    $missing = 0
    $lines = foreach ($entry in $checks.GetEnumerator()) {
        if (Test-Path -LiteralPath $entry.Value -PathType Leaf) {
            "$($entry.Key) — OK"
        } else {
            $missing++
            "$($entry.Key) — brak ($($entry.Value))"
        }
    }
    if ($missing) { Set-Status "Brakuje plików: $missing." 'LightSalmon' }
    else { Set-Status 'Znaleziono wszystkie pliki launchera.' }
    [void][System.Windows.MessageBox]::Show($window, ($lines -join [Environment]::NewLine), 'Test binariów')
})

$btnSteamSync.Add_Click({
    try {
        Require-File $steamScript
        Require-File $gameExe
        $python = Get-Command python.exe -CommandType Application -ErrorAction SilentlyContinue
        $arguments = @($steamScript)
        if (-not $python) {
            $python = Get-Command py.exe -CommandType Application -ErrorAction SilentlyContinue
            if (-not $python) { throw 'Do zapisania skrótu Steam potrzebny jest Python.' }
            $arguments = @('-3', $steamScript)
        }
        $job = Start-LauncherProcess $python.Source $arguments $root -Hidden
        $job.Name = 'Integracja Steam'
        $jobs.steam = $job
        $btnSteamSync.IsEnabled = $false
        Set-Status 'Zapisywanie skrótu Steam…'
    } catch { Set-Status $_.Exception.Message 'LightSalmon' }
})

$btnLaunch.Add_Click({
    try {
        Require-File $gameExe
        $plan = Get-LaunchPlan
        Require-File $plan.File
        $job = Start-LauncherProcess $plan.File $plan.Arguments $plan.Directory $plan.Environment -Hidden:(-not $chkShowConsole.IsChecked)
        $job.Name = $plan.Name
        $jobs.game = $job
        Reset-LiveAnalyzer $plan.Name
        $btnLaunch.IsEnabled = $false
        $btnClearCache.IsEnabled = $false
        Set-Status "Uruchomiono proces $($plan.Name) (PID $($job.Process.Id))."
    } catch { Set-Status $_.Exception.Message 'LightSalmon' }
})

$window.ShowDialog() | Out-Null
