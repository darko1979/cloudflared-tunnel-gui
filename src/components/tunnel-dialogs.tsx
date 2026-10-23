import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useForm } from 'react-hook-form';
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import type { Tunnel, TunnelRoute } from '@/types/api';

interface CreateTunnelFormData {
    name: string;
    hostname: string;
    protocol: 'http' | 'https';
    localHost: string;
    localPort: number;
}

export interface CreateTunnelData {
    name: string;
    makeActive: boolean;
    route?: TunnelRoute;
}

interface CreateTunnelDialogProps {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    onSubmit: (data: CreateTunnelData) => void;
    isBusy: boolean;
}

const createDefaults: CreateTunnelFormData = {
    name: '',
    hostname: '',
    protocol: 'http',
    localHost: '127.0.0.1',
    localPort: 8080,
};

export function CreateTunnelDialog({ open, onOpenChange, onSubmit, isBusy }: CreateTunnelDialogProps) {
    const { t } = useTranslation();
    const [makeActive, setMakeActive] = useState(true);
    const [noTLSVerify, setNoTLSVerify] = useState(true);
    const [overwriteDns, setOverwriteDns] = useState(true);

    const { register, handleSubmit, reset, watch, setValue, formState: { errors } } = useForm<CreateTunnelFormData>({
        defaultValues: createDefaults,
    });

    const hostname = watch('hostname');
    const protocol = watch('protocol');
    const hasRoute = !!hostname?.trim();

    useEffect(() => {
        if (open) {
            reset(createDefaults);
            setMakeActive(true);
            setNoTLSVerify(true);
            setOverwriteDns(true);
        }
    }, [open, reset]);

    const onFormSubmit = (data: CreateTunnelFormData) => {
        const route = hasRoute
            ? {
                hostname: data.hostname.trim(),
                service: `${data.protocol}://${data.localHost.trim()}:${Number(data.localPort)}`,
                noTLSVerify: data.protocol === 'https' && noTLSVerify,
                overwriteDns,
            }
            : undefined;
        // A route is stored in config.yml, so it implies using this tunnel there
        onSubmit({ name: data.name.trim(), makeActive: makeActive || hasRoute, route });
    };

    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent className="sm:max-w-[480px]">
                <DialogHeader>
                    <DialogTitle>{t('tunnels.createTitle')}</DialogTitle>
                    <DialogDescription>{t('tunnels.createDescription')}</DialogDescription>
                </DialogHeader>
                <form onSubmit={handleSubmit(onFormSubmit)}>
                    <div className="grid gap-4 py-4">
                        <div className="grid gap-2">
                            <Label htmlFor="tunnel-name">{t('tunnels.name')}</Label>
                            <Input
                                id="tunnel-name"
                                placeholder={t('tunnels.namePlaceholder')}
                                {...register('name', { required: true, pattern: /^\s*[A-Za-z0-9][A-Za-z0-9._-]*\s*$/ })}
                            />
                            {errors.name ? (
                                <p className="text-sm text-destructive">{t('tunnels.nameInvalid')}</p>
                            ) : (
                                <p className="text-xs text-muted-foreground">{t('tunnels.nameHint')}</p>
                            )}
                        </div>

                        <div className="grid gap-2">
                            <Label htmlFor="tunnel-hostname">{t('tunnels.publicHostname')}</Label>
                            <Input
                                id="tunnel-hostname"
                                placeholder={t('form.hostnamePlaceholder')}
                                {...register('hostname', {
                                    pattern: /^\s*(?=.{1,253}$)([A-Za-z0-9-]{1,63}\.)+[A-Za-z]{2,63}\s*$/,
                                })}
                            />
                            {errors.hostname ? (
                                <p className="text-sm text-destructive">{t('tunnels.hostnameInvalid')}</p>
                            ) : (
                                <p className="text-xs text-muted-foreground">{t('tunnels.publicHostnameHint')}</p>
                            )}
                        </div>

                        <div className="grid gap-2">
                            <Label>{t('tunnels.localService')}</Label>
                            <div className="flex gap-2">
                                <Select
                                    value={protocol}
                                    onValueChange={(value) => setValue('protocol', value as 'http' | 'https')}
                                >
                                    <SelectTrigger className="w-[110px] shrink-0">
                                        <SelectValue />
                                    </SelectTrigger>
                                    <SelectContent>
                                        <SelectItem value="http">http://</SelectItem>
                                        <SelectItem value="https">https://</SelectItem>
                                    </SelectContent>
                                </Select>
                                <Input
                                    aria-label={t('form.localHost')}
                                    placeholder={t('form.localHostPlaceholder')}
                                    {...register('localHost', { required: hasRoute, pattern: /^\s*[^\s/:]+\s*$/ })}
                                />
                                <Input
                                    aria-label={t('form.localPort')}
                                    type="number"
                                    className="w-[110px] shrink-0"
                                    {...register('localPort', { required: hasRoute, min: 1, max: 65535 })}
                                />
                            </div>
                            {errors.localHost || errors.localPort ? (
                                <p className="text-sm text-destructive">{t('tunnels.localServiceInvalid')}</p>
                            ) : (
                                <p className="text-xs text-muted-foreground">{t('tunnels.localServiceHint')}</p>
                            )}
                        </div>

                        {protocol === 'https' && (
                            <div className="flex items-center justify-between gap-4">
                                <Label htmlFor="tunnel-no-tls-verify" className="font-normal">{t('tunnels.noTLSVerify')}</Label>
                                <Switch id="tunnel-no-tls-verify" checked={noTLSVerify} onCheckedChange={setNoTLSVerify} />
                            </div>
                        )}
                        {hasRoute && (
                            <div className="flex items-center justify-between gap-4">
                                <Label htmlFor="tunnel-overwrite-dns" className="font-normal">{t('tunnels.overwriteDns')}</Label>
                                <Switch id="tunnel-overwrite-dns" checked={overwriteDns} onCheckedChange={setOverwriteDns} />
                            </div>
                        )}

                        <div className="flex items-center justify-between gap-4">
                            <Label htmlFor="tunnel-make-active" className="font-normal">{t('tunnels.makeActive')}</Label>
                            <Switch
                                id="tunnel-make-active"
                                checked={makeActive || hasRoute}
                                onCheckedChange={setMakeActive}
                                disabled={hasRoute}
                            />
                        </div>
                    </div>
                    <DialogFooter>
                        <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
                            {t('form.cancel')}
                        </Button>
                        <Button type="submit" disabled={isBusy}>{t('tunnels.create')}</Button>
                    </DialogFooter>
                </form>
            </DialogContent>
        </Dialog>
    );
}

interface UpdateTunnelDialogProps {
    tunnel: Tunnel | null;
    onOpenChange: (open: boolean) => void;
    onSubmit: (data: { makeActive: boolean; restoreCredentials: boolean }) => void;
    isBusy: boolean;
}

export function UpdateTunnelDialog({ tunnel, onOpenChange, onSubmit, isBusy }: UpdateTunnelDialogProps) {
    const { t } = useTranslation();
    const [makeActive, setMakeActive] = useState(false);
    const [restoreCredentials, setRestoreCredentials] = useState(false);

    useEffect(() => {
        if (tunnel) {
            setMakeActive(!!tunnel.isActive);
            setRestoreCredentials(!tunnel.hasCredentials);
        }
    }, [tunnel]);

    return (
        <Dialog open={!!tunnel} onOpenChange={onOpenChange}>
            <DialogContent className="sm:max-w-[480px]">
                <DialogHeader>
                    <DialogTitle>{t('tunnels.updateTitle', { name: tunnel?.name })}</DialogTitle>
                    <DialogDescription>{t('tunnels.updateDescription')}</DialogDescription>
                </DialogHeader>
                <div className="grid gap-4 py-4">
                    <div className="grid gap-1 text-sm">
                        <span className="text-muted-foreground">{t('tunnels.id')}</span>
                        <code className="font-mono text-xs break-all">{tunnel?.id}</code>
                    </div>
                    <div className="grid gap-1 text-sm">
                        <span className="text-muted-foreground">{t('tunnels.credentialsFile')}</span>
                        <code className="font-mono text-xs break-all">{tunnel?.credentialsFile}</code>
                        {!tunnel?.hasCredentials && (
                            <span className="text-xs text-yellow-600 dark:text-yellow-500">{t('tunnels.credentialsMissing')}</span>
                        )}
                    </div>
                    <div className="flex items-center justify-between gap-4">
                        <Label htmlFor="tunnel-update-active" className="font-normal">{t('tunnels.makeActive')}</Label>
                        <Switch id="tunnel-update-active" checked={makeActive} onCheckedChange={setMakeActive} />
                    </div>
                    <div className="flex items-center justify-between gap-4">
                        <Label htmlFor="tunnel-restore-creds" className="font-normal">{t('tunnels.restoreCredentials')}</Label>
                        <Switch id="tunnel-restore-creds" checked={restoreCredentials} onCheckedChange={setRestoreCredentials} />
                    </div>
                    <p className="text-xs text-muted-foreground">{t('tunnels.renameUnsupported')}</p>
                </div>
                <DialogFooter>
                    <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
                        {t('form.cancel')}
                    </Button>
                    <Button onClick={() => onSubmit({ makeActive, restoreCredentials })} disabled={isBusy}>
                        {t('form.save')}
                    </Button>
                </DialogFooter>
            </DialogContent>
        </Dialog>
    );
}

interface DeleteTunnelDialogProps {
    tunnel: Tunnel | null;
    onOpenChange: (open: boolean) => void;
    onConfirm: (force: boolean) => void;
    isBusy: boolean;
}

export function DeleteTunnelDialog({ tunnel, onOpenChange, onConfirm, isBusy }: DeleteTunnelDialogProps) {
    const { t } = useTranslation();
    const [force, setForce] = useState(false);

    useEffect(() => {
        if (tunnel) setForce(false);
    }, [tunnel]);

    return (
        <Dialog open={!!tunnel} onOpenChange={onOpenChange}>
            <DialogContent className="sm:max-w-[480px]">
                <DialogHeader>
                    <DialogTitle>{t('tunnels.deleteTitle', { name: tunnel?.name })}</DialogTitle>
                    <DialogDescription>{t('tunnels.deleteConfirm')}</DialogDescription>
                </DialogHeader>
                <div className="grid gap-4 py-4 text-sm">
                    {tunnel?.isActive && (
                        <p className="text-yellow-600 dark:text-yellow-500">{t('tunnels.deleteActiveWarning')}</p>
                    )}
                    <p className="text-muted-foreground">{t('tunnels.deleteDnsNote')}</p>
                    <div className="flex items-center justify-between gap-4">
                        <Label htmlFor="tunnel-delete-force" className="font-normal">{t('tunnels.forceDelete')}</Label>
                        <Switch id="tunnel-delete-force" checked={force} onCheckedChange={setForce} />
                    </div>
                </div>
                <DialogFooter>
                    <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
                        {t('form.cancel')}
                    </Button>
                    <Button variant="destructive" onClick={() => onConfirm(force)} disabled={isBusy}>
                        {t('table.delete')}
                    </Button>
                </DialogFooter>
            </DialogContent>
        </Dialog>
    );
}
