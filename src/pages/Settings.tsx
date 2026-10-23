import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { getVersion } from '@tauri-apps/api/app';
import { LogIn, LogOut, Check, X } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Label } from '@/components/ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Badge } from '@/components/ui/badge';
import { useTheme } from '@/components/theme-provider';
import { useAppStore } from '@/store/app-store';
import { languages } from '@/i18n';
import { toast } from 'sonner';

export default function Settings() {
    const { t, i18n } = useTranslation();
    const { theme, setTheme } = useTheme();
    const [isLoggingIn, setIsLoggingIn] = useState(false);
    const [isLoggingOut, setIsLoggingOut] = useState(false);
    const [version, setVersion] = useState('');

    const { isAuthenticated, setAuthenticated } = useAppStore();

    useEffect(() => {
        getVersion().then(setVersion).catch(() => setVersion(''));
    }, []);

    useEffect(() => {
        const checkAuth = async () => {
            if (!window.api) return;
            try {
                const result = await window.api.checkAuth();
                setAuthenticated(result.authenticated);
            } catch (error) {
                console.error('Failed to check auth:', error);
            }
        };
        checkAuth();
    }, [setAuthenticated]);

    const handleLanguageChange = (value: string) => {
        i18n.changeLanguage(value);
        localStorage.setItem('language', value);
    };

    const handleLogin = async () => {
        if (!window.api) return;

        setIsLoggingIn(true);
        try {
            const result = await window.api.login();
            if (result.success) {
                setAuthenticated(true);
                toast.success(t('settings.loginSuccess'));
            } else {
                toast.error(result.error || t('settings.loginFailed'));
            }
        } catch (error) {
            toast.error(t('settings.loginFailed'));
        } finally {
            setIsLoggingIn(false);
        }
    };

    const handleLogout = async () => {
        if (!window.api) return;

        setIsLoggingOut(true);
        try {
            const result = await window.api.logout();
            if (result.success) {
                setAuthenticated(false);
                toast.success(t('settings.logoutSuccess'));
            } else {
                toast.error(result.error || t('settings.logoutFailed'));
            }
        } catch (error) {
            toast.error(t('settings.logoutFailed'));
        } finally {
            setIsLoggingOut(false);
        }
    };

    return (
        <div className="space-y-6">
            {/* Header */}
            <div>
                <h1 className="text-2xl font-bold">{t('settings.title')}</h1>
            </div>

            {/* Language Settings */}
            <Card>
                <CardHeader>
                    <CardTitle className="text-lg">{t('settings.language')}</CardTitle>
                </CardHeader>
                <CardContent>
                    <div className="flex items-center gap-4">
                        <Label htmlFor="language" className="min-w-[100px]">{t('settings.language')}</Label>
                        <Select value={i18n.language} onValueChange={handleLanguageChange}>
                            <SelectTrigger className="w-[200px]">
                                <SelectValue />
                            </SelectTrigger>
                            <SelectContent>
                                {languages.map(({ code, label }) => (
                                    <SelectItem key={code} value={code}>{label}</SelectItem>
                                ))}
                            </SelectContent>
                        </Select>
                    </div>
                </CardContent>
            </Card>

            {/* Theme Settings */}
            <Card>
                <CardHeader>
                    <CardTitle className="text-lg">{t('settings.theme')}</CardTitle>
                </CardHeader>
                <CardContent>
                    <div className="flex items-center gap-4">
                        <Label htmlFor="theme" className="min-w-[100px]">{t('settings.theme')}</Label>
                        <Select value={theme} onValueChange={(value: 'light' | 'dark' | 'system') => setTheme(value)}>
                            <SelectTrigger className="w-[200px]">
                                <SelectValue />
                            </SelectTrigger>
                            <SelectContent>
                                <SelectItem value="light">{t('settings.themeLight')}</SelectItem>
                                <SelectItem value="dark">{t('settings.themeDark')}</SelectItem>
                                <SelectItem value="system">{t('settings.themeSystem')}</SelectItem>
                            </SelectContent>
                        </Select>
                    </div>
                </CardContent>
            </Card>

            {/* Auth Settings */}
            <Card>
                <CardHeader>
                    <CardTitle className="text-lg">{t('settings.auth')}</CardTitle>
                    <CardDescription>{t('settings.authDescription')}</CardDescription>
                </CardHeader>
                <CardContent className="space-y-4">
                    <div className="flex items-center gap-4">
                        <Label className="min-w-[100px]">{t('settings.authStatus')}</Label>
                        <Badge variant={isAuthenticated ? 'default' : 'secondary'} className="gap-1">
                            {isAuthenticated ? (
                                <>
                                    <Check className="h-3 w-3" />
                                    {t('settings.authenticated')}
                                </>
                            ) : (
                                <>
                                    <X className="h-3 w-3" />
                                    {t('settings.notAuthenticated')}
                                </>
                            )}
                        </Badge>
                    </div>
                    <div className="flex gap-2">
                        {!isAuthenticated ? (
                            <Button onClick={handleLogin} disabled={isLoggingIn}>
                                <LogIn className="h-4 w-4 mr-2" />
                                {t('settings.login')}
                            </Button>
                        ) : (
                            <Button onClick={handleLogout} disabled={isLoggingOut} variant="destructive">
                                <LogOut className="h-4 w-4 mr-2" />
                                {t('settings.logout')}
                            </Button>
                        )}
                    </div>
                </CardContent>
            </Card>

            {/* About */}
            <Card>
                <CardHeader>
                    <CardTitle className="text-lg">{t('settings.about')}</CardTitle>
                </CardHeader>
                <CardContent>
                    <div className="flex items-center gap-4">
                        <Label className="min-w-[100px]">{t('settings.version')}</Label>
                        <span className="text-sm text-muted-foreground">{version || '-'}</span>
                    </div>
                </CardContent>
            </Card>
        </div>
    );
}
