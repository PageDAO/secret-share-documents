'use client';

import {useFieldArray, useForm} from "react-hook-form"
import { z } from "zod"
import { zodResolver } from "@hookform/resolvers/zod"
import { Plus, Share2 } from "lucide-react";
import {Button} from "@/components/ui/button";
import {Input} from "@/components/ui/input";
import {
    Form,
    FormControl,
    FormField,
    FormItem,
    FormLabel,
    FormMessage,
} from "@/components/ui/form"
import {
    DialogHeader,
    DialogTitle,
    DialogDescription,
    DialogFooter
} from "@/components/ui/dialog";
import {
    Select,
    SelectTrigger,
    SelectItem,
    SelectValue,
    SelectContent
} from "@/components/ui/select"
import {useContext, useState} from "react";
import { SecretDocumentContext } from "@/context/SecretDocumentContext";
import { useToast } from "@/components/ui/use-toast";

interface Props {
    fileIds: string[];
    fileAccess: Array<{ address: string, permission: string }>;
}

const FormSchema = z.object({
    access: z.array(z.object({
            address: z.string().regex(
                /^secret1[a-z0-9]{38}$/,
                { message: "Invalid address" }
            ),
            permission: z.string(),
        })
    ).nonempty(),
})

const accessControl = [
    { label: "Can view", value: "addViewing" },
    { label: "Remove access", value: "deleteViewing" },
    { label: "Owner", value: "changeOwner" },
];

export function FileIdShareForm({fileId, fileAccess}: Props) {
    const { client } = useContext(SecretDocumentContext);
    const { toast } = useToast();
    const [loading, setLoading] = useState(false);

    const form = useForm<z.infer<typeof FormSchema>>({
        resolver: zodResolver(FormSchema),
        defaultValues: {
            access: fileAccess
        }
    })


    function onSubmit(data: z.infer<typeof FormSchema>) {
        setLoading(true);

        const addressesToDelete = data.access
            .filter(access => access.permission === "deleteViewing")
            .map(access => {
                return access.address
            });

        const addressesToAdd = data.access
            .filter(access => access.permission === "addViewing")
            .map(access => {
                return access.address
            });

        const addressesToChangeOwner = data.access
            .filter(access => access.permission === "changeOwner")
            .map(access => {
                return access.address
            });

        const shareFileAccess = {
            changeOwner: addressesToChangeOwner[0],
            addViewing: addressesToAdd,
            deleteViewing: addressesToDelete,
        };

        console.log({shareFileAccess})

        try{
            client.shareDocument(fileId).share(shareFileAccess);
        } catch (error) {
            console.error(error);
        } finally {
            setLoading(false);
        }
    }

    const {fields, append} = useFieldArray({
        control: form.control,
        name: "access",
    });

    console.log(form.formState.errors);

    return(
        <Form {...form}>
            <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-6">
                <DialogHeader>
                    <DialogTitle>Share with</DialogTitle>
                </DialogHeader>
                <div className="flex flex-col items-start gap-4 py-4">
                        {fields.map((field, index) => {
                            console.log(field);

                            return(
                                <div key={field.id} className="grid grid-cols-2 w-full items-center gap-4">
                                    <FormField
                                        control={form.control}
                                        name={`access.${index}.address`}
                                        render={({ field }) => (
                                            <FormItem className="flex flex-col">
                                                <FormLabel>Address</FormLabel>
                                                <FormControl>
                                                    <Input
                                                        className="w-full"
                                                        placeholder="secret1…"
                                                        disabled={fileAccess.some(access => access.address === field.value)}
                                                        {...field}
                                                    />
                                                </FormControl>
                                                <FormMessage />
                                            </FormItem>
                                        )}
                                    />
                                    <FormField
                                        key={field.id}
                                        control={form.control}
                                        name={`access.${index}.permission`}
                                        render={({ field }) => (
                                            <FormItem className="flex flex-col">
                                                <FormLabel>Access Control</FormLabel>
                                                <Select onValueChange={field.onChange} defaultValue={field.value}>
                                                    <FormControl>
                                                        <SelectTrigger className="w-full">
                                                            <SelectValue placeholder="Select rights" />
                                                        </SelectTrigger>
                                                    </FormControl>
                                                    <SelectContent className="w-full">
                                                        { accessControl.map((permission) => (
                                                            <SelectItem
                                                                value={permission.value}
                                                                key={permission.value}
                                                            >
                                                                {permission.label}
                                                            </SelectItem>
                                                        )) }
                                                    </SelectContent>
                                                </Select>
                                                {/*<FormDescription>*/}
                                                {/*    This is the language that will be used in the dashboard.*/}
                                                {/*</FormDescription>*/}
                                                <FormMessage />
                                            </FormItem>
                                        )}
                                    />
                                </div>
                            )
                        })}
                        <Button size="sm" variant="link" className="gap-2" onClick={() => {
                            append({ address: "", permission: "" })
                        }}>
                            <Plus className="w-4 h-4 flex-none"/>
                            Add new account
                        </Button>
                    <DialogDescription>
                        Make changes to your shares here. You can edit addresses with access ad click save when you&apos;re done.
                    </DialogDescription>
                </div>
                <DialogFooter>

                    <Button disabled={loading} type="submit">{
                        loading ? "Saving…" : "Save changes"
                    }</Button>
                </DialogFooter>
            </form>
        </Form>
    )
}
