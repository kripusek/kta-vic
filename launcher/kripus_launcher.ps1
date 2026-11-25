Add-Type -AssemblyName PresentationFramework, PresentationCore, WindowsBase

[xml]$xaml = @'
<Window xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
        xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
        Title="KRIPUS LAUNCHER — Grand Theft Auto VI (VI Causeway)"
        Height="780" Width="1040"
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

                        <!-- Preset 1 (Working): ScarlettPort Lit World -->
                        <RadioButton x:Name="optScarlettPort" Style="{StaticResource PresetCardStyle}" IsChecked="True">
                            <Grid>
                                <Grid.ColumnDefinitions>
                                    <ColumnDefinition Width="Auto"/>
                                    <ColumnDefinition Width="*"/>
                                    <ColumnDefinition Width="Auto"/>
                                </Grid.ColumnDefinitions>
                                <TextBlock Text="🌴" FontSize="18" VerticalAlignment="Center" Margin="0,0,12,0"/>
                                <StackPanel Grid.Column="1">
                                    <TextBlock Text="ScarlettPort v0.3.1 (Lit World + KBM + Own Upscaler)" FontSize="14" FontWeight="Bold" Foreground="#FFFFFF"/>
                                    <TextBlock Text="Oświetlony świat (fx-cand / precompiled), klawiatura+mysz, AutoInput, Agility SDK" FontSize="11" Foreground="#94A3B8" Margin="0,2,0,0"/>
                                </StackPanel>
                                <Border Grid.Column="2" Background="#0F332B" CornerRadius="6" Padding="6,3" VerticalAlignment="Center">
                                    <TextBlock Text="ZALECANY" FontSize="9" FontWeight="Black" Foreground="#10B981"/>
                                </Border>
                            </Grid>
                        </RadioButton>

                        <!-- Preset 2 (Working): ScarlettPort FastBoot -->
                        <RadioButton x:Name="optScarlettPortDebug" Style="{StaticResource PresetCardStyle}">
                            <Grid>
                                <Grid.ColumnDefinitions>
                                    <ColumnDefinition Width="Auto"/>
                                    <ColumnDefinition Width="*"/>
                                    <ColumnDefinition Width="Auto"/>
                                </Grid.ColumnDefinitions>
                                <TextBlock Text="🚀" FontSize="18" VerticalAlignment="Center" Margin="0,0,12,0"/>
                                <StackPanel Grid.Column="1">
                                    <TextBlock Text="ScarlettPort — FastBoot + ReBAR (Szybki start do świata)" FontSize="14" FontWeight="Bold" Foreground="#FFFFFF"/>
                                    <TextBlock Text="Pomija logo/menu (-FastBoot -AutoInput) + VICTORY_REBAR=1 + KBM + Own Upscaler" FontSize="11" Foreground="#94A3B8" Margin="0,2,0,0"/>
                                </StackPanel>
                                <Border Grid.Column="2" Background="#1D294D" CornerRadius="6" Padding="6,3" VerticalAlignment="Center">
                                    <TextBlock Text="FASTBOOT" FontSize="9" FontWeight="Black" Foreground="#60A5FA"/>
                                </Border>
                            </Grid>
                        </RadioButton>

                        <!-- Preset 3 (Working): fxceless/victory-fork -->
                        <RadioButton x:Name="optVictoryFork" Style="{StaticResource PresetCardStyle}">
                            <Grid>
                                <Grid.ColumnDefinitions>
                                    <ColumnDefinition Width="Auto"/>
                                    <ColumnDefinition Width="*"/>
                                    <ColumnDefinition Width="Auto"/>
                                </Grid.ColumnDefinitions>
                                <TextBlock Text="⚡" FontSize="18" VerticalAlignment="Center" Margin="0,0,12,0"/>
                                <StackPanel Grid.Column="1">
                                    <TextBlock Text="fxceless/victory-fork (~10 FPS + Indirect Gather)" FontSize="14" FontWeight="Bold" Foreground="#FFFFFF"/>
                                    <TextBlock Text="Oświetlony prolog 8-10 FPS (Series X, VICTORY_INDIRECT_GATHER=1, pełny zestaw flag)" FontSize="11" Foreground="#94A3B8" Margin="0,2,0,0"/>
                                </StackPanel>
                                <Border Grid.Column="2" Background="#2B153D" CornerRadius="6" Padding="6,3" VerticalAlignment="Center">
                                    <TextBlock Text="10 FPS" FontSize="9" FontWeight="Black" Foreground="#D946EF"/>
                                </Border>
                            </Grid>
                        </RadioButton>

                        <!-- Preset 4 (Working): Belobug AMD Fork -->
                        <RadioButton x:Name="optBelobugAmd" Style="{StaticResource PresetCardStyle}">
                            <Grid>
                                <Grid.ColumnDefinitions>
                                    <ColumnDefinition Width="Auto"/>
                                    <ColumnDefinition Width="*"/>
                                    <ColumnDefinition Width="Auto"/>
                                </Grid.ColumnDefinitions>
                                <TextBlock Text="🔥" FontSize="18" VerticalAlignment="Center" Margin="0,0,12,0"/>
                                <StackPanel Grid.Column="1">
                                    <TextBlock Text="Belobug AMD Fork (Ambient Light + FPS Overlay)" FontSize="14" FontWeight="Bold" Foreground="#FFFFFF"/>
                                    <TextBlock Text="Oświetlony świat, pojazdy i dalekie drzewa (VICTORY_AMBIENT=0.6, VICTORY_HAIR=1)" FontSize="11" Foreground="#94A3B8" Margin="0,2,0,0"/>
                                </StackPanel>
                                <Border Grid.Column="2" Background="#3D1D14" CornerRadius="6" Padding="6,3" VerticalAlignment="Center">
                                    <TextBlock Text="BELOBUG" FontSize="9" FontWeight="Black" Foreground="#FB923C"/>
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
                                    <TextBlock Text="Victory Corgi — Xbox Series X" FontSize="14" FontWeight="Bold" Foreground="#FFFFFF"/>
                                    <TextBlock Text="Po starcie ekran może być chwilowo czarny. Gdy pojawi się PLAYER, naciśnij Enter lub A." FontSize="11" Foreground="#94A3B8" Margin="0,2,0,0" TextWrapping="Wrap"/>
                                </StackPanel>
                                <Border Grid.Column="2" Background="#3B1D2E" CornerRadius="6" Padding="6,3" VerticalAlignment="Center">
                                    <TextBlock Text="CORGI" FontSize="9" FontWeight="Black" Foreground="#F472B6"/>
                                </Border>
                            </Grid>
                        </RadioButton>

                        <TextBlock Text="⚠️ NIE DZIAŁAJĄCE GRAFICZNIE (CZARNY ŚWIAT PO INTRO / DEBUG)" FontSize="13" FontWeight="Bold" Foreground="#F59E0B" Margin="0,12,0,10"/>

                        <!-- Preset 6 (Non-working world): Series S 1080p -->
                        <RadioButton x:Name="optSeriesS1080" Style="{StaticResource PresetCardStyle}">
                            <Grid>
                                <Grid.ColumnDefinitions>
                                    <ColumnDefinition Width="Auto"/>
                                    <ColumnDefinition Width="*"/>
                                    <ColumnDefinition Width="Auto"/>
                                </Grid.ColumnDefinitions>
                                <TextBlock Text="🎮" FontSize="18" VerticalAlignment="Center" Margin="0,0,12,0"/>
                                <StackPanel Grid.Column="1">
                                    <TextBlock Text="VI Causeway / kta-vic — Xbox Series S @ 1080p 60 FPS" FontSize="14" FontWeight="Bold" Foreground="#FFFFFF"/>
                                    <TextBlock Text="Działa intro i menu; po kartach ładowania świat pozostaje ciemny" FontSize="11" Foreground="#94A3B8" Margin="0,2,0,0"/>
                                </StackPanel>
                                <Border Grid.Column="2" Background="#25283E" CornerRadius="6" Padding="6,3" VerticalAlignment="Center">
                                    <TextBlock Text="MENU OK" FontSize="9" FontWeight="Black" Foreground="#94A3B8"/>
                                </Border>
                            </Grid>
                        </RadioButton>

                        <!-- Preset 7 (Non-working world): Series S Unlocked -->
                        <RadioButton x:Name="optSeriesSUnlocked" Style="{StaticResource PresetCardStyle}">
                            <Grid>
                                <Grid.ColumnDefinitions>
                                    <ColumnDefinition Width="Auto"/>
                                    <ColumnDefinition Width="*"/>
                                    <ColumnDefinition Width="Auto"/>
                                </Grid.ColumnDefinitions>
                                <TextBlock Text="🔓" FontSize="18" VerticalAlignment="Center" Margin="0,0,12,0"/>
                                <StackPanel Grid.Column="1">
                                    <TextBlock Text="VI Causeway / kta-vic — Xbox Series S @ UNLOCKED FPS" FontSize="14" FontWeight="Bold" Foreground="#FFFFFF"/>
                                    <TextBlock Text="Brak limitu 60 FPS (świat 3D nadal nieoświetlony)" FontSize="11" Foreground="#94A3B8" Margin="0,2,0,0"/>
                                </StackPanel>
                                <Border Grid.Column="2" Background="#25283E" CornerRadius="6" Padding="6,3" VerticalAlignment="Center">
                                    <TextBlock Text="NO LIGHT" FontSize="9" FontWeight="Black" Foreground="#94A3B8"/>
                                </Border>
                            </Grid>
                        </RadioButton>

                        <!-- Preset 8 (Non-working world): Series X 1080p -->
                        <RadioButton x:Name="optSeriesX1080" Style="{StaticResource PresetCardStyle}">
                            <Grid>
                                <Grid.ColumnDefinitions>
                                    <ColumnDefinition Width="Auto"/>
                                    <ColumnDefinition Width="*"/>
                                </Grid.ColumnDefinitions>
                                <TextBlock Text="🌟" FontSize="18" VerticalAlignment="Center" Margin="0,0,12,0"/>
                                <StackPanel Grid.Column="1">
                                    <TextBlock Text="VI Causeway / kta-vic — Xbox Series X @ 1080p 60 FPS" FontSize="14" FontWeight="Bold" Foreground="#FFFFFF"/>
                                    <TextBlock Text="Profil Series X ze skalowaniem do 1080p (brak oświetlenia świata)" FontSize="11" Foreground="#94A3B8" Margin="0,2,0,0"/>
                                </StackPanel>
                            </Grid>
                        </RadioButton>

                        <!-- Preset 9 (Non-working world): Series X 4K -->
                        <RadioButton x:Name="optSeriesX4K" Style="{StaticResource PresetCardStyle}">
                            <Grid>
                                <Grid.ColumnDefinitions>
                                    <ColumnDefinition Width="Auto"/>
                                    <ColumnDefinition Width="*"/>
                                </Grid.ColumnDefinitions>
                                <TextBlock Text="💎" FontSize="18" VerticalAlignment="Center" Margin="0,0,12,0"/>
                                <StackPanel Grid.Column="1">
                                    <TextBlock Text="VI Causeway / kta-vic — Xbox Series X @ Natywne 4K (2160p)" FontSize="14" FontWeight="Bold" Foreground="#FFFFFF"/>
                                    <TextBlock Text="Pełna rozdzielczość 4K (duże zużycie VRAM, świat nieoświetlony)" FontSize="11" Foreground="#94A3B8" Margin="0,2,0,0"/>
                                </StackPanel>
                            </Grid>
                        </RadioButton>

                        <!-- Preset 10 (Debug): Diag -->
                        <RadioButton x:Name="optDiag" Style="{StaticResource PresetCardStyle}">
                            <Grid>
                                <Grid.ColumnDefinitions>
                                    <ColumnDefinition Width="Auto"/>
                                    <ColumnDefinition Width="*"/>
                                    <ColumnDefinition Width="Auto"/>
                                </Grid.ColumnDefinitions>
                                <TextBlock Text="🛠️" FontSize="18" VerticalAlignment="Center" Margin="0,0,12,0"/>
                                <StackPanel Grid.Column="1">
                                    <TextBlock Text="Tryb Diagnostyczny (Target \diag\victory.exe)" FontSize="14" FontWeight="Bold" Foreground="#FFFFFF"/>
                                    <TextBlock Text="Włączona telemetria shaderów, DRED i trace loader do analizy błędów" FontSize="11" Foreground="#94A3B8" Margin="0,2,0,0"/>
                                </StackPanel>
                                <Border Grid.Column="2" Background="#36220B" CornerRadius="6" Padding="6,3" VerticalAlignment="Center">
                                    <TextBlock Text="DEBUG" FontSize="9" FontWeight="Black" Foreground="#F59E0B"/>
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
                        <RowDefinition Height="*"/>
                        <RowDefinition Height="Auto"/>
                    </Grid.RowDefinitions>

                    <TextBlock Grid.Row="0" Text="OPCJE SILNIKA I GRAFIKI" FontSize="14" FontWeight="Bold" Foreground="#FF007F" Margin="0,0,0,12"/>

                    <StackPanel Grid.Row="1" Margin="0,0,0,12">
                        <CheckBox x:Name="chkClzero" Content="Aktywne buforowanie CLZERO (Wysoki FPS + Ochrona RAM)" IsChecked="True" Foreground="#CBD5E1" Margin="0,0,0,8" Cursor="Hand"/>
                        <CheckBox x:Name="chkSafeSubmit" Content="Tryb Safe Submit (Tylko diagnostyka; obniża FPS!)" IsChecked="False" Foreground="#94A3B8" Margin="0,0,0,8" Cursor="Hand"/>
                        <CheckBox x:Name="chkDred" Content="D3D12 DRED &amp; Breadcrumbs (Narzut debuggera)" IsChecked="False" Foreground="#94A3B8" Margin="0,0,0,8" Cursor="Hand"/>
                        <CheckBox x:Name="chkMaxWorkers" Content="Limit workerów shaderów: 2 (Dla słabszych CPU)" IsChecked="False" Foreground="#94A3B8" Margin="0,0,0,8" Cursor="Hand"/>
                        <CheckBox x:Name="chkShowConsole" Content="Wyświetlaj konsolę na żywo podczas gry" IsChecked="True" Foreground="#CBD5E1" Margin="0,0,0,8" Cursor="Hand"/>
                    </StackPanel>

                    <!-- Quick Tools & Actions -->
                    <Border Grid.Row="2" Background="#121422" BorderBrush="#1F2338" BorderThickness="1" CornerRadius="8" Padding="12">
                        <StackPanel>
                            <TextBlock Text="SZYBKIE NARZĘDZIA" FontSize="11" FontWeight="Bold" Foreground="#8A92B2" Margin="0,0,0,8"/>
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
                            
                            <TextBlock x:Name="txtToolStatus" Text="System gotowy. Wybierz profil i kliknij ODPAL." FontSize="11" Foreground="#38EF7D" Margin="0,10,0,0" TextWrapping="Wrap"/>
                        </StackPanel>
                    </Border>

                    <!-- Hint / Footnote -->
                    <TextBlock Grid.Row="3" Text="Wskazówka: Po ostatnich poprawkach shaderów (Wave32 + SRV raw buffer), profil Series S 1080p gwarantuje płynne przejście przez cutscenki do otwartego świata."
                               FontSize="10.5" Foreground="#64748B" TextWrapping="Wrap" Margin="0,10,0,0"/>
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
$optSeriesS1080 = $window.FindName("optSeriesS1080")
$optSeriesSUnlocked = $window.FindName("optSeriesSUnlocked")
$optSeriesX1080 = $window.FindName("optSeriesX1080")
$optSeriesX4K = $window.FindName("optSeriesX4K")
$optDiag = $window.FindName("optDiag")
$optScarlettPort = $window.FindName("optScarlettPort")
$optScarlettPortDebug = $window.FindName("optScarlettPortDebug")
$optVictoryFork = $window.FindName("optVictoryFork")
$optBelobugAmd = $window.FindName("optBelobugAmd")
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
$victoryDir = Join-Path $root 'victory'
$corgiDir = Join-Path $root 'victory-corgi'
$belobugDir = Join-Path $root 'victory-belobug'
$forkDir = Join-Path $root 'victory-fork'
$scarlettDir = Join-Path $root 'ScarlettPort'
$releaseExe = Join-Path $victoryDir 'target\release\victory.exe'
$diagExe = Join-Path $victoryDir 'target\diag\victory.exe'
$corgiExe = Join-Path $corgiDir 'target\release\victory.exe'
$belobugExe = Join-Path $belobugDir 'target\release\victory.exe'
$forkExe = Join-Path $forkDir 'target\release\victory.exe'
$scarlettExe = Join-Path $scarlettDir 'runtime\victory\target\release\victory.exe'
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

    if ($optScarlettPort.IsChecked -or $optScarlettPortDebug.IsChecked) {
        Require-File $scarlettExe
        Require-File $scarlettScript
        $profile = 'ScarlettPort'
        $exe = Join-Path $PSHOME 'powershell.exe'
        $workDir = $scarlettDir
        $arguments = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $scarlettScript,
            '-Game', $gameDir, '-Console', 'series-s', '-AutoInput')
        $environment.VICTORY_FALLBACK_LIGHT = $null
        $environment.VICTORY_REBAR = $null
        $environment.VICTORY_FAST_BOOT = $null
        if ($optScarlettPortDebug.IsChecked) {
            $profile = 'ScarlettPort FastBoot'
            $environment.VICTORY_REBAR = '1'
            $arguments += '-FastBoot'
        }
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
        $arguments = @('run', $gameExe, '--execute', '--console', 'series-x')
    } elseif ($optBelobugAmd.IsChecked) {
        $profile = 'Belobug AMD Fork'
        $exe = $belobugExe
        $workDir = $belobugDir
        $environment.VICTORY_AMBIENT = '0.6'
        $environment.VICTORY_HAIR = '1'
        $environment.VICTORY_PRESENT_TARGET = '1x1:1'
        $arguments = @('run', $gameExe, '--execute', '--console', 'series-x')
    } elseif ($optVictoryCorgi.IsChecked) {
        $profile = 'Victory Corgi Series X'
        $exe = $corgiExe
        $workDir = $corgiDir
        $arguments = @('run', $gameExe, '--execute', '--console', 'series-x')
    } else {
        $environment.VICTORY_GPU_TIMING = '1'
        if ($optDiag.IsChecked) {
            $profile = 'VIctory diagnostyczny'
            $exe = $diagExe
            $environment.VICTORY_D3D12_DEBUG = '1'
            $environment.VICTORY_D3D12_DRED = '1'
            $environment.VICTORY_LOG = 'victory_loader=trace,victory_gpu=info,info'
        } elseif ($optSeriesSUnlocked.IsChecked) {
            $profile = 'VIctory Series S bez limitu FPS'
            $environment.VICTORY_UNLOCK_FPS = '1'
        } elseif ($optSeriesX1080.IsChecked -or $optSeriesX4K.IsChecked) {
            $console = 'series-x'
            $profile = 'VIctory Series X 1080p'
            if ($optSeriesX4K.IsChecked) {
                $profile = 'VIctory Series X 4K'
                $height = $null
            }
        }
        if ($chkClzero.IsChecked) { $environment.VICTORY_BATCH_CLZERO = '1' }
        if ($chkSafeSubmit.IsChecked) { $environment.VICTORY_SAFE_SUBMIT = '1' }
        if (-not $optDiag.IsChecked) {
            $environment.VICTORY_D3D12_DRED = if ($chkDred.IsChecked) { '1' } else { '0' }
        }

        $arguments = @('run', $gameExe, '--unimplemented', 'zero', '--console', $console)
        if ($optSeriesSUnlocked.IsChecked -ne $true) { $arguments += @('--fps', '60') }
        if ($height) { $arguments += @('--output-height', $height) }
        if ($chkMaxWorkers.IsChecked) { $arguments += @('--max-pipeline-workers', '2') }
    }
    return @{ Name = $profile; File = $exe; Arguments = $arguments; Directory = $workDir; Environment = $environment }
}

$refreshEngineOptions = {
    $isVictory = -not ($optVictoryCorgi.IsChecked -or $optScarlettPort.IsChecked -or $optScarlettPortDebug.IsChecked -or $optBelobugAmd.IsChecked -or $optVictoryFork.IsChecked)
    $chkClzero.IsEnabled = $isVictory
    $chkSafeSubmit.IsEnabled = $isVictory
    $chkMaxWorkers.IsEnabled = $isVictory
    $chkDred.IsEnabled = $isVictory -and -not $optDiag.IsChecked
}
foreach ($preset in @($optSeriesS1080, $optSeriesSUnlocked, $optSeriesX1080, $optSeriesX4K,
        $optDiag, $optScarlettPort, $optScarlettPortDebug, $optVictoryFork, $optBelobugAmd, $optVictoryCorgi)) {
    $preset.Add_Checked($refreshEngineOptions)
}
& $refreshEngineOptions

$timer = New-Object System.Windows.Threading.DispatcherTimer
$timer.Interval = [TimeSpan]::FromMilliseconds(500)
$timer.Add_Tick({
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
        'VIctory Release' = $releaseExe
        'VIctory Diag' = $diagExe
        'Victory Corgi' = $corgiExe
        'Belobug AMD' = $belobugExe
        'fxceless victory-fork' = $forkExe
        'ScarlettPort' = $scarlettExe
        'Skrypt ScarlettPort' = $scarlettScript
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
        Require-File $releaseExe
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
        $btnLaunch.IsEnabled = $false
        $btnClearCache.IsEnabled = $false
        Set-Status "Uruchomiono proces $($plan.Name) (PID $($job.Process.Id))."
    } catch { Set-Status $_.Exception.Message 'LightSalmon' }
})

$window.ShowDialog() | Out-Null
