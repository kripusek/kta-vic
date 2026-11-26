Add-Type -AssemblyName PresentationFramework, PresentationCore, WindowsBase

[xml]$xaml = @'
<Window xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
        xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
        Title="KRIPUS LAUNCHER — Grand Theft Auto VI (VI Causeway)"
        Height="840" Width="1100"
        WindowStartupLocation="CenterScreen"
        ResizeMode="CanMinimize"
        Background="#0A0B12"
        Foreground="#ECEFF4"
        FontFamily="Segoe UI, Segoe UI Variable, sans-serif">
    
    <Window.Resources>
        <!-- Vice City Cyberpunk Gradients & Colors -->
        <LinearGradientBrush x:Key="ViceGradient" StartPoint="0,0" EndPoint="1,1">
            <GradientStop Color="#00F0FF" Offset="0.0"/>
            <GradientStop Color="#7928CA" Offset="0.5"/>
            <GradientStop Color="#FF007F" Offset="1.0"/>
        </LinearGradientBrush>

        <LinearGradientBrush x:Key="CardGradient" StartPoint="0,0" EndPoint="1,1">
            <GradientStop Color="#161826" Offset="0.0"/>
            <GradientStop Color="#10111D" Offset="1.0"/>
        </LinearGradientBrush>

        <LinearGradientBrush x:Key="CardHoverGradient" StartPoint="0,0" EndPoint="1,1">
            <GradientStop Color="#202338" Offset="0.0"/>
            <GradientStop Color="#18192B" Offset="1.0"/>
        </LinearGradientBrush>

        <LinearGradientBrush x:Key="LaunchButtonGradient" StartPoint="0,0" EndPoint="1,0">
            <GradientStop Color="#FF007F" Offset="0.0"/>
            <GradientStop Color="#9B00E8" Offset="0.6"/>
            <GradientStop Color="#00F0FF" Offset="1.0"/>
        </LinearGradientBrush>

        <!-- Drop Shadows -->
        <DropShadowEffect x:Key="GlowCyan" Color="#00F0FF" BlurRadius="20" ShadowDepth="0" Opacity="0.45"/>
        <DropShadowEffect x:Key="GlowPink" Color="#FF007F" BlurRadius="25" ShadowDepth="0" Opacity="0.55"/>
        <DropShadowEffect x:Key="SoftShadow" Color="#000000" BlurRadius="15" ShadowDepth="4" Opacity="0.6"/>

        <!-- Custom Card RadioButton Style -->
        <Style x:Key="PresetCardStyle" TargetType="RadioButton">
            <Setter Property="Background" Value="{StaticResource CardGradient}"/>
            <Setter Property="BorderBrush" Value="#25283E"/>
            <Setter Property="BorderThickness" Value="1"/>
            <Setter Property="Foreground" Value="#ECEFF4"/>
            <Setter Property="Margin" Value="0,0,0,10"/>
            <Setter Property="Cursor" Value="Hand"/>
            <Setter Property="Template">
                <Setter.Value>
                    <ControlTemplate TargetType="RadioButton">
                        <Border x:Name="border"
                                Background="{TemplateBinding Background}"
                                BorderBrush="{TemplateBinding BorderBrush}"
                                BorderThickness="{TemplateBinding BorderThickness}"
                                CornerRadius="10"
                                Padding="16,12"
                                Effect="{StaticResource SoftShadow}">
                            <ContentPresenter />
                        </Border>
                        <ControlTemplate.Triggers>
                            <Trigger Property="IsMouseOver" Value="True">
                                <Setter TargetName="border" Property="Background" Value="{StaticResource CardHoverGradient}"/>
                                <Setter TargetName="border" Property="BorderBrush" Value="#00F0FF"/>
                            </Trigger>
                            <Trigger Property="IsChecked" Value="True">
                                <Setter TargetName="border" Property="BorderBrush" Value="#FF007F"/>
                                <Setter TargetName="border" Property="BorderThickness" Value="1.8"/>
                                <Setter TargetName="border" Property="Effect" Value="{StaticResource GlowPink}"/>
                                <Setter TargetName="border" Property="Background" Value="#1C182F"/>
                            </Trigger>
                        </ControlTemplate.Triggers>
                    </ControlTemplate>
                </Setter.Value>
            </Setter>
        </Style>

        <!-- Action Button Style -->
        <Style x:Key="ActionButtonStyle" TargetType="Button">
            <Setter Property="Background" Value="#181A2A"/>
            <Setter Property="BorderBrush" Value="#2D3150"/>
            <Setter Property="BorderThickness" Value="1"/>
            <Setter Property="Foreground" Value="#E2E8F0"/>
            <Setter Property="FontSize" Value="13"/>
            <Setter Property="FontWeight" Value="SemiBold"/>
            <Setter Property="Padding" Value="14,10"/>
            <Setter Property="Cursor" Value="Hand"/>
            <Setter Property="Template">
                <Setter.Value>
                    <ControlTemplate TargetType="Button">
                        <Border x:Name="btnBorder"
                                Background="{TemplateBinding Background}"
                                BorderBrush="{TemplateBinding BorderBrush}"
                                BorderThickness="{TemplateBinding BorderThickness}"
                                CornerRadius="8"
                                Padding="{TemplateBinding Padding}">
                            <ContentPresenter HorizontalAlignment="Center" VerticalAlignment="Center"/>
                        </Border>
                        <ControlTemplate.Triggers>
                            <Trigger Property="IsMouseOver" Value="True">
                                <Setter TargetName="btnBorder" Property="BorderBrush" Value="#00F0FF"/>
                                <Setter TargetName="btnBorder" Property="Background" Value="#22253B"/>
                            </Trigger>
                            <Trigger Property="IsPressed" Value="True">
                                <Setter TargetName="btnBorder" Property="Background" Value="#141624"/>
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

        <!-- 1. Header -->
        <Grid Grid.Row="0" Margin="0,0,0,14">
            <Grid.ColumnDefinitions>
                <ColumnDefinition Width="*"/>
                <ColumnDefinition Width="Auto"/>
            </Grid.ColumnDefinitions>

            <StackPanel Orientation="Vertical">
                <StackPanel Orientation="Horizontal" VerticalAlignment="Center">
                    <TextBlock Text="⚡ KRIPUS" FontSize="26" FontWeight="Black" Foreground="#FF007F"/>
                    <TextBlock Text=" LAUNCHER" FontSize="26" FontWeight="Black" Foreground="#00F0FF"/>
                    <Border Background="#251336" CornerRadius="4" Margin="14,0,0,0" Padding="8,2" VerticalAlignment="Center" BorderBrush="#FF007F" BorderThickness="1">
                        <TextBlock Text="GTA VI • VI CAUSEWAY" FontSize="11" FontWeight="Bold" Foreground="#FF80DF"/>
                    </Border>
                </StackPanel>
                <TextBlock Text="Xbox Scarlett Compatibility Recompiler • PC Direct3D 12 Edition"
                           FontSize="13" Foreground="#8A92B2" Margin="0,4,0,0"/>
            </StackPanel>

            <StackPanel Grid.Column="1" Orientation="Horizontal" VerticalAlignment="Center">
                <Border Background="#121320" BorderBrush="#25283E" BorderThickness="1" CornerRadius="8" Padding="12,6" Margin="0,0,8,0">
                    <StackPanel Orientation="Horizontal">
                        <TextBlock Text="🎮 GRA: " FontWeight="Bold" Foreground="#8A92B2" FontSize="12"/>
                        <TextBlock Text="GTA VI 1.0.2.1" FontWeight="Bold" Foreground="#38EF7D" FontSize="12"/>
                    </StackPanel>
                </Border>
                <Border Background="#121320" BorderBrush="#25283E" BorderThickness="1" CornerRadius="8" Padding="12,6">
                    <StackPanel Orientation="Horizontal">
                        <TextBlock Text="👤 USER: " FontWeight="Bold" Foreground="#8A92B2" FontSize="12"/>
                        <TextBlock Text="kripusek" FontWeight="Bold" Foreground="#00F0FF" FontSize="12"/>
                    </StackPanel>
                </Border>
            </StackPanel>
        </Grid>

        <!-- 2. System Status Bar -->
        <Border Grid.Row="1" Background="#11121E" BorderBrush="#1F2236" BorderThickness="1" CornerRadius="10" Padding="14,10" Margin="0,0,0,18">
            <Grid>
                <Grid.ColumnDefinitions>
                    <ColumnDefinition Width="*"/>
                    <ColumnDefinition Width="*"/>
                    <ColumnDefinition Width="*"/>
                    <ColumnDefinition Width="*"/>
                </Grid.ColumnDefinitions>

                <StackPanel Grid.Column="0">
                    <TextBlock Text="KARTA GRAFICZNA" FontSize="10" FontWeight="Bold" Foreground="#70789E"/>
                    <TextBlock x:Name="txtGPU" Text="NVIDIA RTX 3060 Ti" FontSize="13" FontWeight="SemiBold" Foreground="#00F0FF" Margin="0,2,0,0"/>
                </StackPanel>

                <StackPanel Grid.Column="1">
                    <TextBlock Text="ROZMIAR FALI SHADERÓW" FontSize="10" FontWeight="Bold" Foreground="#70789E"/>
                    <TextBlock Text="Wave32 (PR #578 Safe)" FontSize="13" FontWeight="SemiBold" Foreground="#38EF7D" Margin="0,2,0,0"/>
                </StackPanel>

                <StackPanel Grid.Column="2">
                    <TextBlock Text="STAN KOMPILATORA" FontSize="10" FontWeight="Bold" Foreground="#70789E"/>
                    <TextBlock Text="causeway-integration (OK)" FontSize="13" FontWeight="SemiBold" Foreground="#E2B714" Margin="0,2,0,0"/>
                </StackPanel>

                <StackPanel Grid.Column="3">
                    <TextBlock Text="TESTY SHADERÓW / GPU" FontSize="10" FontWeight="Bold" Foreground="#70789E"/>
                    <TextBlock Text="243/243 ZALICZONE" FontSize="13" FontWeight="SemiBold" Foreground="#A78BFA" Margin="0,2,0,0"/>
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
            <Border Grid.Column="0" Background="#0E101B" BorderBrush="#1E2033" BorderThickness="1" CornerRadius="12" Padding="18" Margin="0,0,12,0">
                <ScrollViewer VerticalScrollBarVisibility="Auto">
                    <StackPanel Orientation="Vertical">
                        <TextBlock Text="✅ DZIAŁAJĄCE GRAFICZNIE (OŚWIETLONY ŚWIAT / 3D)" FontSize="13" FontWeight="Bold" Foreground="#38EF7D" Margin="0,0,0,10"/>

                        <!-- Preset 0 (Working): CGBJ pareto-residency + OWN_UPSCALER -->
                        <RadioButton x:Name="optCgbj" Style="{StaticResource PresetCardStyle}" IsChecked="True">
                            <Grid>
                                <Grid.ColumnDefinitions>
                                    <ColumnDefinition Width="Auto"/>
                                    <ColumnDefinition Width="*"/>
                                    <ColumnDefinition Width="Auto"/>
                                </Grid.ColumnDefinitions>
                                <TextBlock Text="🦊" FontSize="18" VerticalAlignment="Center" Margin="0,0,12,0"/>
                                <StackPanel Grid.Column="1">
                                    <TextBlock Text="CGBJ / cool_fox (pareto-residency + OWN_UPSCALER=1)" FontSize="14" FontWeight="Bold" Foreground="#FFFFFF"/>
                                    <TextBlock Text="Sprawdzony na RTX 3060 Ti: najwyższy FPS + własny upscaler gry (bez crasha FSR4)" FontSize="11" Foreground="#94A3B8" Margin="0,2,0,0"/>
                                </StackPanel>
                                <Border Grid.Column="2" Background="#0C3423" CornerRadius="6" Padding="6,3" VerticalAlignment="Center">
                                    <TextBlock Text="NAJLEPSZY" FontSize="9" FontWeight="Black" Foreground="#38EF7D"/>
                                </Border>
                            </Grid>
                        </RadioButton>

                        <!-- Preset 1 (Working): DoubleGhosty/cool_fox -->
                        <RadioButton x:Name="optCoolFox" Style="{StaticResource PresetCardStyle}">
                            <Grid>
                                <Grid.ColumnDefinitions>
                                    <ColumnDefinition Width="Auto"/>
                                    <ColumnDefinition Width="*"/>
                                    <ColumnDefinition Width="Auto"/>
                                </Grid.ColumnDefinitions>
                                <TextBlock Text="🦊" FontSize="18" VerticalAlignment="Center" Margin="0,0,12,0"/>
                                <StackPanel Grid.Column="1">
                                    <TextBlock Text="DoubleGhosty/cool_fox (CGBJ Continuation)" FontSize="14" FontWeight="Bold" Foreground="#FFFFFF"/>
                                    <TextBlock Text="Pareto residency, LOD fix, sync wideo NVIDIA + VICTORY_OWN_UPSCALER=1" FontSize="11" Foreground="#94A3B8" Margin="0,2,0,0"/>
                                </StackPanel>
                                <Border Grid.Column="2" Background="#3A1530" CornerRadius="6" Padding="6,3" VerticalAlignment="Center">
                                    <TextBlock Text="COOL_FOX" FontSize="9" FontWeight="Black" Foreground="#FF007F"/>
                                </Border>
                            </Grid>
                        </RadioButton>

                        <!-- Preset 3 (Working): ScarlettPort v0.4.5 Look + FastBoot -->
                        <RadioButton x:Name="optScarlettPortDebug" Style="{StaticResource PresetCardStyle}">
                            <Grid>
                                <Grid.ColumnDefinitions>
                                    <ColumnDefinition Width="Auto"/>
                                    <ColumnDefinition Width="*"/>
                                    <ColumnDefinition Width="Auto"/>
                                </Grid.ColumnDefinitions>
                                <TextBlock Text="🌴" FontSize="18" VerticalAlignment="Center" Margin="0,0,12,0"/>
                                <StackPanel Grid.Column="1">
                                    <TextBlock Text="ScarlettPort v0.4.5 (Look + FastBoot + ReBAR)" FontSize="14" FontWeight="Bold" Foreground="#FFFFFF"/>
                                    <TextBlock Text="Nowe poprawki obrazu/shaderów (-Look -FastBoot -AutoInput) + REBAR=1 + scarlett.exe" FontSize="11" Foreground="#94A3B8" Margin="0,2,0,0"/>
                                </StackPanel>
                                <Border Grid.Column="2" Background="#1D294D" CornerRadius="6" Padding="6,3" VerticalAlignment="Center">
                                    <TextBlock Text="v0.4.5" FontSize="9" FontWeight="Black" Foreground="#60A5FA"/>
                                </Border>
                            </Grid>
                        </RadioButton>

                        <!-- Preset 4 (Working): fxceless/victory-fork -->
                        <RadioButton x:Name="optVictoryFork" Style="{StaticResource PresetCardStyle}">
                            <Grid>
                                <Grid.ColumnDefinitions>
                                    <ColumnDefinition Width="Auto"/>
                                    <ColumnDefinition Width="*"/>
                                    <ColumnDefinition Width="Auto"/>
                                </Grid.ColumnDefinitions>
                                <TextBlock Text="⚡" FontSize="18" VerticalAlignment="Center" Margin="0,0,12,0"/>
                                <StackPanel Grid.Column="1">
                                    <TextBlock Text="fxceless/victory-fork (e4245f3 • Black-Face + Mouths Fix)" FontSize="14" FontWeight="Bold" Foreground="#FFFFFF"/>
                                    <TextBlock Text="Naprawa czarnych twarzy, ruszające się usta (PR #927), -10 ms/klatkę, series-s" FontSize="11" Foreground="#94A3B8" Margin="0,2,0,0"/>
                                </StackPanel>
                                <Border Grid.Column="2" Background="#2B153D" CornerRadius="6" Padding="6,3" VerticalAlignment="Center">
                                    <TextBlock Text="LATEST" FontSize="9" FontWeight="Black" Foreground="#D946EF"/>
                                </Border>
                            </Grid>
                        </RadioButton>

                        <!-- Preset 5 (Working): Victory Corgi -->
                        <RadioButton x:Name="optVictoryCorgi" Style="{StaticResource PresetCardStyle}">
                            <Grid>
                                <Grid.ColumnDefinitions>
                                    <ColumnDefinition Width="Auto"/>
                                    <ColumnDefinition Width="*"/>
                                    <ColumnDefinition Width="Auto"/>
                                </Grid.ColumnDefinitions>
                                <TextBlock Text="🐶" FontSize="18" VerticalAlignment="Center" Margin="0,0,12,0"/>
                                <StackPanel Grid.Column="1">
                                    <TextBlock Text="Victory Corgi (8b6671cc • Series X)" FontSize="14" FontWeight="Bold" Foreground="#FFFFFF"/>
                                    <TextBlock Text="Na starcie ekran bywa czarny; gdy pojawi się PLAYER, naciśnij Enter lub A (Series X)" FontSize="11" Foreground="#94A3B8" Margin="0,2,0,0"/>
                                </StackPanel>
                                <Border Grid.Column="2" Background="#3B1D2E" CornerRadius="6" Padding="6,3" VerticalAlignment="Center">
                                    <TextBlock Text="CORGI" FontSize="9" FontWeight="Black" Foreground="#F472B6"/>
                                </Border>
                            </Grid>
                        </RadioButton>

                    </StackPanel>
                </ScrollViewer>
            </Border>

            <!-- Tweaks & Diagnostics Section -->
            <Border Grid.Column="1" Background="#0E101B" BorderBrush="#1E2033" BorderThickness="1" CornerRadius="12" Padding="18">
                <Grid>
                    <Grid.RowDefinitions>
                        <RowDefinition Height="Auto"/>
                        <RowDefinition Height="Auto"/>
                        <RowDefinition Height="Auto"/>
                        <RowDefinition Height="*"/>
                    </Grid.RowDefinitions>

                    <TextBlock Grid.Row="0" Text="OPCJE SILNIKA I GRAFIKI" FontSize="14" FontWeight="Bold" Foreground="#FF007F" Margin="0,0,0,12"/>

                    <StackPanel Grid.Row="1" Margin="0,0,0,12">
                        <CheckBox x:Name="chkClzero" Content="Aktywne buforowanie CLZERO (Wysoki FPS + Ochrona RAM)" IsChecked="True" Foreground="#CBD5E1" Margin="0,0,0,8" Cursor="Hand"/>
                        <CheckBox x:Name="chkSafeSubmit" Content="Tryb Safe Submit (Tylko diagnostyka; obniża FPS!)" IsChecked="False" Foreground="#94A3B8" Margin="0,0,0,8" Cursor="Hand"/>
                        <CheckBox x:Name="chkDred" Content="D3D12 DRED &amp; Breadcrumbs (Narzut debuggera)" IsChecked="False" Foreground="#94A3B8" Margin="0,0,0,8" Cursor="Hand"/>
                        <CheckBox x:Name="chkMaxWorkers" Content="Limit workerów shaderów: 2 (Dla słabszych CPU)" IsChecked="False" Foreground="#94A3B8" Margin="0,0,0,8" Cursor="Hand"/>
                        <CheckBox x:Name="chkShowConsole" Content="Osobne okno konsoli (bez podglądu stdout w analizatorze)" IsChecked="False" Foreground="#CBD5E1" Margin="0,0,0,8" Cursor="Hand"/>
                    </StackPanel>

                    <!-- Quick Tools & Actions -->
                    <Border Grid.Row="2" Background="#121422" BorderBrush="#1F2338" BorderThickness="1" CornerRadius="8" Padding="10" Margin="0,0,0,10">
                        <StackPanel>
                            <TextBlock Text="SZYBKIE NARZĘDZIA" FontSize="11" FontWeight="Bold" Foreground="#8A92B2" Margin="0,0,0,6"/>
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
                            
                            <TextBlock x:Name="txtToolStatus" Text="System gotowy. Wybierz profil i kliknij ODPAL." FontSize="11" Foreground="#38EF7D" Margin="0,8,0,0" TextWrapping="Wrap"/>
                        </StackPanel>
                    </Border>

                    <!-- Real-Time Live Game Analyzer -->
                    <Border Grid.Row="3" Background="#0A0C15" BorderBrush="#252A44" BorderThickness="1" CornerRadius="8" Padding="10">
                        <Grid>
                            <Grid.RowDefinitions>
                                <RowDefinition Height="Auto"/>
                                <RowDefinition Height="Auto"/>
                                <RowDefinition Height="Auto"/>
                                <RowDefinition Height="*"/>
                            </Grid.RowDefinitions>

                            <Grid Grid.Row="0" Margin="0,0,0,6">
                                <TextBlock Text="📊 ANALIZATOR GRY NA ŻYWO" FontSize="11" FontWeight="Bold" Foreground="#00F0FF" HorizontalAlignment="Left"/>
                                <TextBlock x:Name="txtLiveStage" Text="⏸ Oczekiwanie na start" FontSize="11" FontWeight="Bold" Foreground="#94A3B8" HorizontalAlignment="Right"/>
                            </Grid>

                            <Grid Grid.Row="1" Margin="0,0,0,6">
                                <Grid.ColumnDefinitions>
                                    <ColumnDefinition Width="*"/>
                                    <ColumnDefinition Width="*"/>
                                </Grid.ColumnDefinitions>
                                <StackPanel Grid.Column="0" Margin="0,0,6,0">
                                    <TextBlock Text="FPS / KLATKA" FontSize="9" FontWeight="Bold" Foreground="#64748B"/>
                                    <TextBlock x:Name="txtLiveFps" Text="--" FontSize="12" FontWeight="Bold" Foreground="#38EF7D"/>
                                </StackPanel>
                                <StackPanel Grid.Column="1">
                                    <TextBlock Text="RAM PROCESU" FontSize="9" FontWeight="Bold" Foreground="#64748B"/>
                                    <TextBlock x:Name="txtLiveRam" Text="--" FontSize="12" FontWeight="Bold" Foreground="#A78BFA"/>
                                </StackPanel>
                            </Grid>

                            <Grid Grid.Row="2" Margin="0,0,0,6">
                                <Grid.ColumnDefinitions>
                                    <ColumnDefinition Width="*"/>
                                    <ColumnDefinition Width="*"/>
                                </Grid.ColumnDefinitions>
                                <StackPanel Grid.Column="0" Margin="0,0,6,0">
                                    <TextBlock Text="SHADERY / POTOKI" FontSize="9" FontWeight="Bold" Foreground="#64748B"/>
                                    <TextBlock x:Name="txtLiveShaders" Text="0" FontSize="12" FontWeight="Bold" Foreground="#E2B714"/>
                                </StackPanel>
                                <StackPanel Grid.Column="1">
                                    <TextBlock Text="WARN / BŁĘDY" FontSize="9" FontWeight="Bold" Foreground="#64748B"/>
                                    <TextBlock x:Name="txtLiveErrors" Text="0 / 0" FontSize="12" FontWeight="Bold" Foreground="#94A3B8"/>
                                </StackPanel>
                            </Grid>

                            <TextBox x:Name="txtLiveLog" Grid.Row="3"
                                     IsReadOnly="True"
                                     Background="#07080F"
                                     Foreground="#CBD5E1"
                                     BorderBrush="#1B1E30"
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
            </Grid.ColumnDefinitions>

            <Button x:Name="btnLaunch" Grid.Column="0" Height="68" Margin="0,0,12,0" Cursor="Hand" BorderThickness="0">
                <Button.Template>
                    <ControlTemplate TargetType="Button">
                        <Border x:Name="launchBorder"
                                Background="{StaticResource LaunchButtonGradient}"
                                CornerRadius="12"
                                Effect="{StaticResource GlowPink}">
                            <StackPanel Orientation="Horizontal" HorizontalAlignment="Center" VerticalAlignment="Center">
                                <TextBlock Text="▶" FontSize="26" Foreground="#FFFFFF" VerticalAlignment="Center" Margin="0,0,14,0"/>
                                <StackPanel VerticalAlignment="Center">
                                    <TextBlock Text="ODPAL GTA VI" FontSize="20" FontWeight="Black" Foreground="#FFFFFF"/>
                                    <TextBlock Text="Uruchom Grand Theft Auto VI przez VI Causeway" FontSize="11" Foreground="#FDF2F8"/>
                                </StackPanel>
                            </StackPanel>
                        </Border>
                        <ControlTemplate.Triggers>
                            <Trigger Property="IsMouseOver" Value="True">
                                <Setter TargetName="launchBorder" Property="Effect" Value="{StaticResource GlowCyan}"/>
                                <Setter TargetName="launchBorder" Property="Opacity" Value="0.95"/>
                            </Trigger>
                            <Trigger Property="IsPressed" Value="True">
                                <Setter TargetName="launchBorder" Property="Opacity" Value="0.8"/>
                            </Trigger>
                        </ControlTemplate.Triggers>
                    </ControlTemplate>
                </Button.Template>
            </Button>

            <Button x:Name="btnExit" Grid.Column="1" Width="100" Height="68" Content="WYJŚCIE" Style="{StaticResource ActionButtonStyle}" FontSize="14" FontWeight="Bold"/>
        </Grid>
    </Grid>
</Window>
'@

# Load XAML
$reader = New-Object System.Xml.XmlNodeReader $xaml
$window = [System.Windows.Markup.XamlReader]::Load($reader)

# Element References
$optCgbj = $window.FindName("optCgbj")
$optCoolFox = $window.FindName("optCoolFox")
$optScarlettPortDebug = $window.FindName("optScarlettPortDebug")
$optVictoryFork = $window.FindName("optVictoryFork")
$optVictoryCorgi = $window.FindName("optVictoryCorgi")

$chkClzero = $window.FindName("chkClzero")
$chkSafeSubmit = $window.FindName("chkSafeSubmit")
$chkDred = $window.FindName("chkDred")
$chkMaxWorkers = $window.FindName("chkMaxWorkers")
$chkShowConsole = $window.FindName("chkShowConsole")

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
$txtLiveRam = $window.FindName("txtLiveRam")
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
$cgbjDir = Join-Path $root 'victory-cgbj'
$coolFoxDir = Join-Path $root 'victory-coolfox'
$forkDir = Join-Path $root 'victory-fork'
$corgiDir = Join-Path $root 'victory-corgi'
$scarlettDir = Join-Path $root 'ScarlettPort'
$cgbjExe = Join-Path $cgbjDir 'target\release\victory.exe'
$cgbjCmd = Join-Path $cgbjDir 'Play-VIctory.cmd'
$coolFoxExe = Join-Path $coolFoxDir 'target\release\victory.exe'
$coolFoxCmd = Join-Path $coolFoxDir 'Play-VIctory.cmd'
$forkExe = Join-Path $forkDir 'target\release\victory.exe'
$corgiExe = Join-Path $corgiDir 'target\release\victory.exe'
$scarlettExe = Join-Path $scarlettDir 'engine\target\release\scarlett.exe'
$scarlettScript = Join-Path $scarlettDir 'scripts\play-victory.ps1'
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

    if ($optCgbj.IsChecked) {
        Require-File $cgbjExe
        Require-File $cgbjCmd
        $profile = 'CGBJ / cool_fox (pareto-residency)'
        $exe = $env:ComSpec
        $workDir = $cgbjDir
        $environment.VICTORY_GAME_DIR = $gameDir
        $environment.VICTORY_CACHE_DIR = Join-Path $dataDir 'cache-cgbj'
        $environment.VICTORY_OWN_UPSCALER = '1'
        $arguments = @('/c', $cgbjCmd, 'pareto-residency')
    } elseif ($optCoolFox.IsChecked) {
        Require-File $coolFoxExe
        Require-File $coolFoxCmd
        $profile = 'DoubleGhosty/cool_fox'
        $exe = $env:ComSpec
        $workDir = $coolFoxDir
        $environment.VICTORY_GAME_DIR = $gameDir
        $environment.VICTORY_CACHE_DIR = Join-Path $dataDir 'cache-coolfox'
        $environment.VICTORY_OWN_UPSCALER = '1'
        $arguments = @('/c', $coolFoxCmd)
    } elseif ($optScarlettPortDebug.IsChecked) {
        Require-File $scarlettExe
        Require-File $scarlettScript
        $profile = 'ScarlettPort v0.4.5 (Look + FastBoot)'
        $exe = Join-Path $PSHOME 'powershell.exe'
        $workDir = $scarlettDir
        $arguments = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $scarlettScript,
            '-Game', $gameDir, '-Console', 'series-s', '-AutoInput', '-FastBoot', '-Look')
        $environment.SCARLETT_REBAR = '1'
        $environment.SCARLETT_INDIRECT_WINDOW = '1'
        $environment.SCARLETT_INDIRECT_KEEP = '1'
        $environment.SCARLETT_TABLED_SKIP_FRAMES = '2'
    } elseif ($optVictoryFork.IsChecked) {
        $profile = 'fxceless/victory-fork (~10 FPS)'
        $exe = $forkExe
        $workDir = $forkDir
        $environment.VICTORY_LEVELS_ON_DEMAND = '1'
        $environment.VICTORY_ASYNC_SUBMIT = '1'
        $environment.VICTORY_QUEUE2 = '1'
        $environment.VICTORY_INDIRECT_WINDOW = '1'
        $environment.VICTORY_TEXTURE_NO_WAIT = '1'
        $environment.VICTORY_MERGE_GAP = '64'
        $environment.VICTORY_MERGE_RUNS = '4096'
        $environment.VICTORY_STAGING_POOL = '0'
        $environment.VICTORY_COPY_PLAIN = '1'
        $environment.VICTORY_JOIN_PARTS = '1'
        $environment.VICTORY_BACK_OWN_VIEW = '1'
        $environment.VICTORY_INDIRECT_GATHER = '1'
        $environment.VICTORY_CARD_REHASH_RUNS = '1'
        $arguments = @('run', $gameExe, '--execute', '--console', 'series-s')
    } elseif ($optVictoryCorgi.IsChecked) {
        Require-File $corgiExe
        $profile = 'Victory Corgi (8b6671cc)'
        $exe = $corgiExe
        $workDir = $corgiDir
        $arguments = @('run', $gameExe, '--execute', '--console', 'series-x')
    } else {
        throw 'Wybierz jeden z profili graficznych.'
    }
    return @{ Name = $profile; File = $exe; Arguments = $arguments; Directory = $workDir; Environment = $environment }
}

$refreshEngineOptions = {
    $chkClzero.IsEnabled = $false
    $chkSafeSubmit.IsEnabled = $false
    $chkMaxWorkers.IsEnabled = $false
    $chkDred.IsEnabled = $false
}
foreach ($preset in @($optCgbj, $optCoolFox, $optScarlettPortDebug, $optVictoryFork, $optVictoryCorgi)) {
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
    $liveState.Shaders = 0
    $liveState.Warnings = 0
    $liveState.Errors = 0
    $liveState.Lines.Clear()
    $txtLiveStage.Text = "⏳ Inicjalizacja ($ProfileName)..."
    $txtLiveStage.Foreground = '#00F0FF'
    $txtLiveFps.Text = '--'
    $txtLiveRam.Text = '--'
    $txtLiveShaders.Text = '0'
    $txtLiveErrors.Text = '0 / 0'
    $txtLiveErrors.Foreground = '#94A3B8'
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

function Update-LiveAnalyzer {
    $proc = Get-Process -Name victory -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($proc) {
        $gb = [math]::Round($proc.WorkingSet64 / 1GB, 2)
        $txtLiveRam.Text = "$gb GB"
    } elseif (-not $jobs.ContainsKey('game')) {
        $txtLiveRam.Text = '--'
    }

    $job = $jobs['game']
    if (-not $job) { return }

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
        'CGBJ / cool_fox' = $cgbjExe
        'Skrypt CGBJ' = $cgbjCmd
        'cool_fox (DoubleGhosty)' = $coolFoxExe
        'ScarlettPort 0.4.5' = $scarlettExe
        'Skrypt ScarlettPort' = $scarlettScript
        'fxceless victory-fork' = $forkExe
        'Victory Corgi' = $corgiExe
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
